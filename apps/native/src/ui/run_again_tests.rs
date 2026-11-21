//! The window's half of Electron's `a-host-started-run-is-not-retried-as-your-message.test.tsx` (its App.tsx case:
//! the offer is asked of the current run) and of `a-damaged-record-says-so-in-the-thread.test.tsx`, drawn.
use super::*;

fn shown(kind: Option<&str>, prompt: &str, failed: bool, never_started: bool, damaged: bool) -> (String, Vec<String>) {
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_codec".into(), name: "Codec".into(), role: "Engineer".into(), runtime: None, model: None, account: None, busy: false, runs_in: "local".into(), folder: None },
        conversation: Conversation { conversation_id: Some("mission_1".into()), turns: Vec::new() },
        items: Vec::new(), dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot {
            failed,
            never_started,
            damaged,
            turn_metas: vec![ledger::Metadata { mission_id: "mission_1".into(), prompt: prompt.into(), started_by_kind: kind.map(str::to_owned), ..Default::default() }],
            ..Default::default()
        },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    let items = vec![ledger::Item::User(prompt.into()), ledger::Item::Note { level: "error".into(), text: "The CLI refused its arguments.".into() }];
    let mut rt = rune::Runtime::new((s, items), |(s, items), _| message_column(items, "Codec", &Palette::new(true), s),
        (800.0, 1200.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let elements = rt.elements();
    let texts = elements.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
    let keys = elements.into_iter().filter_map(|e| e.key).collect();
    (texts, keys)
}

#[test]
fn the_window_offers_run_it_again_for_your_own_never_started_current_run() {
    let (texts, keys) = shown(None, "Check the migration.", true, true, false);
    assert!(texts.contains("Run it again"), "{texts}");
    assert!(texts.contains("Nothing had started, so running this again cannot repeat anything."), "{texts}");
    assert!(keys.iter().any(|k| k == "run-again-button"), "{keys:?}");
}

#[test]
fn the_window_withholds_run_it_again_for_a_relay_and_says_the_message_is_still_waiting() {
    let (texts, keys) = shown(Some("relay"), "Atlas sent you a message; it is quoted below.", true, true, false);
    assert!(!texts.contains("Run it again"), "{texts}");
    assert!(texts.contains("The message is still waiting."), "{texts}");
    assert!(!keys.iter().any(|k| k == "run-again-button"), "{keys:?}");
}

#[test]
fn a_run_that_got_started_offers_nothing() {
    let (texts, _) = shown(None, "Check the migration.", true, false, false);
    assert!(!texts.contains("Run it again"), "{texts}");
}

#[test]
fn a_damaged_record_says_what_is_missing_and_that_the_file_is_kept() {
    let (texts, _) = shown(None, "Summarize the turns", false, false, true);
    assert!(texts.contains("could not be read, so what came after it is missing here. The file is kept as it is."), "{texts}");
}

#[test]
fn a_record_that_read_whole_says_nothing_of_the_kind() {
    let (texts, _) = shown(None, "Summarize the turns", false, false, false);
    assert!(!texts.contains("could not be read"), "{texts}");
}
