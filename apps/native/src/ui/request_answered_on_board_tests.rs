//! Electron's `a-request-is-answered-on-the-board.test.tsx`, case by case, on the Board the window draws
//! (`board_view`): a command is shown whole with where it runs, Allow once and Deny; anything else opens the
//! conversation; a conversation with no request is the card it was.
use super::*;

const LONG: &str = r#"python -m pytest -q tests/test_prices.py tests/test_cart.py --maxfail=1 --disable-warnings -k "total and not slow and not network""#;

fn board(asks: &[(&str, &str, &str, Option<&str>, Option<&str>)]) -> rune::Runtime<State> {
    let mut s = State::default();
    s.teammates.push(Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Code & Migrations".into(), runtime: Some("codex".into()),
        model: None, account: None, busy: false, runs_in: "edit".into(), folder: None });
    for (id, title) in [("m_run", "Run the price tests"), ("m_edit", "Fix the cart total")] {
        s.recent.push(ConversationRow { id: id.into(), title: title.into(), teammate: "Wren".into(), started_by: "native".into(),
            state: "running".into(), updated: "2026-10-04T05:58:00.000Z".into() });
    }
    s.home.waiting_for.insert("m_run".into(), "Run a command".into());
    s.home.waiting_for.insert("m_edit".into(), "Change src/cart.py".into());
    for (mission, approval, kind, command, where_) in asks {
        s.home.asks.insert((*mission).into(), BoardAsk { approval_id: (*approval).into(), kind: (*kind).into(),
            command: command.map(Into::into), where_: where_.map(Into::into), shown: std::time::Instant::now() });
    }
    let mut rt = rune::Runtime::new(s, |s, _| board_view(s, &Palette::new(true)), (1100.0, 1400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn said(rt: &rune::Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text.or(e.label)).collect()
}

fn ask_marks(rt: &rune::Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with("board-request-")).collect()
}

#[test]
fn a_command_is_shown_whole_with_where_and_allow_once_and_deny() {
    // case: shows a command whole, where it runs, and Allow once and Deny
    let rt = board(&[("m_run", "ap_1", "command", Some(LONG), Some("C:/acme-storefront"))]);
    assert_eq!(ask_marks(&rt), ["board-request-ap_1"]);
    let words = said(&rt);
    // Whole, never the 90-character line the title bar's list uses.
    // Held at its flags with word joiners, but every character of it there.
    assert!(words.iter().any(|w| w.replace('\u{2060}', "") == LONG), "{words:?}");
    assert!(words.iter().any(|w| w == "in C:/acme-storefront"));
    assert!(words.iter().any(|w| w == "Allow once"));
    assert!(words.iter().any(|w| w == "Deny"));
    assert!(words.iter().any(|w| w.contains("wants to run")));
    assert!(!words.iter().any(|w| w.to_lowercase().contains("always")));
}

#[test]
fn anything_it_cannot_show_whole_opens_the_conversation() {
    // case: opens the conversation for anything it cannot show whole
    let mut rt = board(&[("m_edit", "ap_2", "file-change", None, Some("C:/acme-storefront")), ("m_run", "ap_3", "question", None, None)]);
    let words = said(&rt);
    assert!(words.iter().any(|w| w == "Review"));
    assert!(words.iter().any(|w| w == "Answer"));
    assert!(!words.iter().any(|w| w == "Allow once"));
    assert!(words.iter().any(|w| w == "Change src/cart.py"));
    // Review opens that conversation.
    let review = rt.elements().into_iter().find(|e| e.text.as_deref() == Some("Review")).unwrap();
    let (x, y) = (review.rect[0] + review.rect[2] / 2.0, review.rect[1] + review.rect[3] / 2.0);
    rt.state.home.board_open = true;
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert_eq!(rt.state.opening_conversation.as_deref(), Some("m_edit"));
    assert!(!rt.state.home.board_open);
}

#[test]
fn a_conversation_without_a_request_is_the_card_it_was() {
    // case: leaves a conversation without a request as the card it was
    let rt = board(&[]);
    assert!(ask_marks(&rt).is_empty());
    let words = said(&rt);
    assert!(words.iter().any(|w| w == "Run the price tests") && words.iter().any(|w| w == "Fix the cart total"));
    assert!(!words.iter().any(|w| w == "Allow once" || w == "Review" || w == "Answer"));
}
