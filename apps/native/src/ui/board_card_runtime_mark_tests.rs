//! A conversation with no teammate (one another app started through Locust's MCP server, say) wears its runtime's mark
//! on its Board card, as its sidebar row does; it is not an empty ring. Follows BoardScreen.tsx:186-192: `by ===
//! undefined` and a runtime and a model that is not an own route draws `RuntimeMark`; otherwise `.lc-conv__nobody`.
use super::*;

fn board(route: (&str, &str)) -> rune::Runtime<State> {
    let at = (chrono::Utc::now() - chrono::Duration::minutes(5)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut s = State::default();
    s.home.board_open = true;
    s.recent = vec![ConversationRow { id: "m_app".into(), teammate: String::new(), title: "Read the logs".into(),
        started_by: "Claude Code".into(), state: "completed".into(), updated: at.clone() }];
    s.ledger_summaries.insert("m_app".into(), locust_core::ledger::Summary { mission_id: "m_app".into(), title: "Read the logs".into(),
        runtime: route.0.into(), model: route.1.into(), updated: at.clone(), created_at: at, ..Default::default() });
    let mut rt = rune::Runtime::new(s, |s, _| board_view(s, &Palette::new(true)), (1400.0, 1400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn within(rt: &rune::Runtime<State>, key: &str, card: [f32; 4]) -> Option<rune::ElementInfo> {
    rt.elements().into_iter().filter(|e| !e.exiting).find(|e| {
        e.key.as_deref() == Some(key) && e.rect[0] >= card[0] && e.rect[0] + e.rect[2] <= card[0] + card[2] + 0.5
            && e.rect[1] >= card[1] && e.rect[1] + e.rect[3] <= card[1] + card[3] + 0.5
    })
}

#[test]
fn a_card_with_no_teammate_wears_its_runtimes_mark_named_where_the_face_goes() {
    let rt = board(("claude", "claude-opus-5-5"));
    let card = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("board-card-m_app")).expect("the card").rect;
    let mark = within(&rt, "board-card-runtime-m_app", card).expect("Claude's mark on the card, not an empty ring");
    assert_eq!(mark.label.as_deref(), Some("Claude Code"));
    assert!((mark.rect[2] - 20.0).abs() < 0.5 && (mark.rect[3] - 20.0).abs() < 0.5, "in the face's 20: {:?}", mark.rect);
    assert!(mark.rect[0] < card[0] + 24.0, "before the title, where a face goes: {:?} {card:?}", mark.rect);
}

#[test]
fn a_model_of_your_own_keeps_the_empty_ring() {
    let rt = board(("opencode", "own-0a1b2c3d/llama-local"));
    assert!(rt.find("board-card-m_app").is_some());
    assert!(rt.find("board-card-runtime-m_app").is_none(), "an own route wears no runtime mark (isOwnRoute)");
}
