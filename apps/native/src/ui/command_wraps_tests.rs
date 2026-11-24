//! Electron's `a-command-wraps-at-its-spaces.test.tsx`: a command held at its flags (`held_command`), drawn so on the
//! approval card (`approval_card`) and on the Board (`crate::home_panel::board_view`). Electron's `lc-nobreak` span is a word
//! joiner here, drawn between the dashes and the letter and left out of what a copy takes. Commands are Electron's.
use super::*;

const COMMAND: &str = r#""C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -Command 'python -m pytest -q --maxfail=1'"#;

/// What a copy of the drawn text takes: the text without its joiners.
fn copied(shown: &str, holds: &[std::ops::Range<usize>]) -> String {
    let mut out = String::new();
    let mut at = 0;
    for hold in holds {
        out.push_str(&shown[at..hold.start]);
        at = hold.end;
    }
    out.push_str(&shown[at..]);
    out
}

fn held(flag: &str) -> String {
    let dashes = flag.trim_end_matches(|c: char| c != '-');
    format!("{dashes}\u{2060}{}", &flag[dashes.len()..])
}

// case: holds each flag to its dash and changes no character
#[test]
fn each_flag_is_held_to_its_dash_and_no_character_changes() {
    let (shown, holds) = held_command(COMMAND);
    for flag in ["-C", "-m", "-q", "--m"] {
        assert!(shown.contains(&held(flag)), "{flag}: {shown:?}");
    }
    assert_eq!(copied(&shown, &holds), COMMAND);
}

// case: leaves a dash inside a word, a lone dash and a number alone
#[test]
fn a_dash_inside_a_word_and_a_lone_dash_are_left_alone() {
    let plain = "git log --oneline -3 a-b - c";
    let (shown, holds) = held_command(plain);
    assert!(shown.contains(&held("--o")), "{shown:?}");
    assert!(shown.contains(&held("-3")), "{shown:?}");
    assert!(shown.contains("a-b"), "{shown:?}");
    assert!(shown.contains(" - c"), "{shown:?}");
    assert_eq!(holds.len(), 2);
    assert_eq!(copied(&shown, &holds), plain);
}

fn request(kind: approval::Kind, detail: &str) -> approval::Request {
    approval::Request {
        approval_id: "ap_1".into(),
        mission_id: "m_run".into(),
        runtime: "codex".into(),
        kind,
        summary: "Run a command".into(),
        detail: detail.into(),
        cwd: Some("C:/acme-storefront".into()),
        requested_at: "2026-10-27T10:00:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false,
        answer_in: None,
        claude_input: None,
        questions: Vec::new(),
    }
}

fn card_words(request: approval::Request) -> String {
    let mut s = State::default();
    s.pending_approval = Some(request);
    let mut rt = rune::Runtime::new(s, |s, _| col().w_full().child(approval_card(s, &Palette::new(true))), (600.0, 900.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

// case: is drawn so on the approval card, and a connector call is not
#[test]
fn the_approval_card_holds_a_commands_flags_and_not_a_connector_calls() {
    let card = card_words(request(approval::Kind::Command, COMMAND));
    assert!(card.contains(&format!("{}ommand", held("-C"))), "{card}");
    let connector = card_words(request(approval::Kind::Connector, "{\n  \"note\": \"see -x\"\n}"));
    assert!(connector.contains("see -x"), "{connector}");
    assert!(!connector.contains('\u{2060}'), "{connector}");
}

// case: is drawn so on the Board
#[test]
fn the_board_holds_a_commands_flags() {
    let mut s = State::default();
    s.recent = vec![ConversationRow { id: "m_run".into(), teammate: String::new(), title: "Run the tests".into(), started_by: "the person".into(), state: "running".into(), updated: String::new() }];
    s.home.board_open = true;
    s.home.asks.insert("m_run".into(), crate::home_panel::BoardAsk { approval_id: "ap_1".into(), kind: "command".into(), command: Some(COMMAND.into()), where_: Some("C:/acme-storefront".into()), shown: std::time::Instant::now() });
    s.home.waiting_for.insert("m_run".into(), "Run a command".into());
    let mut rt = rune::Runtime::new(s, |s, _| crate::home_panel::board_view(s, &Palette::new(true)), (1200.0, 2000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let board = rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n");
    assert!(board.contains(&format!("{}ommand", held("-C"))), "{board}");
}
