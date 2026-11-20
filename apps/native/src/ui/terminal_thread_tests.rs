//! the-terminal-turns-are-drawn-as-yours.test.tsx, the seams and the thread, drawn headless through the message column.
use super::*;
use locust_core::conversation_turns::TerminalSeam;

fn meta(id: &str, prompt: &str, terminal: bool) -> ledger::Metadata {
    ledger::Metadata {
        mission_id: id.into(),
        prompt: prompt.into(),
        runtime: "claude".into(),
        started_by_kind: terminal.then(|| "terminal".to_owned()),
        ..Default::default()
    }
}
/// Who takes what? (here), two turns typed in Claude Code's terminal, then the turn on screen: back here, or the
/// terminal's own when `newest_in_terminal`.
fn thread(newest_in_terminal: bool) -> String {
    let newest = if newest_in_terminal { meta("mission_4", "Push it.", true) } else { meta("mission_4", "What changed?", false) };
    let metas = vec![meta("mission_1", "Who takes what?", false), meta("mission_2", "Now add tests.", true), meta("mission_3", "Commit them.", true), newest];
    let mut items = Vec::new();
    for (i, m) in metas.iter().enumerate() {
        items.push(ledger::Item::User(m.prompt.clone()));
        items.push(ledger::Item::Assistant { item_id: format!("a{i}"), text: format!("Answer {i}."), r#final: true });
    }
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Engineer".into(), runtime: None, model: None, account: None, busy: false, runs_in: "local".into(), folder: None },
        conversation: Conversation { conversation_id: Some("mission_4".into()), turns: Vec::new() },
        items: Vec::new(), dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot { turn_metas: metas, ..Default::default() },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    let mut rt = rune::Runtime::new((s, items), |(s, items), _| message_column(items, "Wren", &Palette::new(true), s), (800.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let mut drawn: Vec<_> = rt.elements().into_iter().filter(|e| e.text.is_some()).collect();
    drawn.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
    drawn.into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}
fn at(html: &str, words: &str) -> usize {
    html.find(words).unwrap_or_else(|| panic!("no {words:?} in\n{html}"))
}

#[test]
fn say_whose_terminal_and_that_it_ran_with_that_runtimes_own_permissions() {
    let (title, note) = crate::terminal_divider::words(&TerminalSeam::Into("claude".into()));
    assert_eq!(title, "In Claude Code's terminal");
    assert!(note.unwrap().contains("Claude Code ran it with its own permissions, not this conversation's mode."));
    assert_eq!(crate::terminal_divider::words(&TerminalSeam::Back), ("Back in Locust".to_owned(), None));
}

#[test]
fn draws_the_terminal_turns_between_one_seam_going_in_and_one_coming_back_the_words_in_order() {
    let html = thread(false);
    let into = at(&html, "In Claude Code's terminal");
    let back = at(&html, "Back in Locust");
    assert!(into > at(&html, "Who takes what?"));
    assert!(at(&html, "Now add tests.") > into);
    assert!(at(&html, "Commit them.") > at(&html, "Now add tests."));
    assert!(back > at(&html, "Commit them."));
    assert!(at(&html, "What changed?") > back);
    assert_eq!(html.matches("In Claude Code's terminal").count(), 1);
}

#[test]
fn opens_on_a_conversation_whose_newest_turn_was_the_terminals_with_the_seam_going_in_and_none_coming_back() {
    let html = thread(true);
    assert_eq!(html.matches("In Claude Code's terminal").count(), 1);
    assert!(!html.contains("Back in Locust"));
}
