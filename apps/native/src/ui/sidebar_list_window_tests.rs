//! A LONG LIST IS DRAWN AS FAR AS IT IS READ (`renderer/src/a-long-list-is-drawn-as-far-as-it-is-read.test.tsx`):
//! the sidebar draws one page (`LIST_PAGE = 80`) of ungrouped conversations newest first, says how many more there are
//! (and pressing that button draws another page), always draws far enough to hold the one open, and draws the whole
//! list with no footer when it fits in a page.
use super::*;
use rune::{Env, Runtime};

fn make_rows(count: usize) -> Vec<ConversationRow> {
    let base = chrono::DateTime::parse_from_rfc3339("2026-10-15T00:00:00Z").unwrap();
    (0..count)
        .map(|index| {
            let at = base - chrono::Duration::minutes(index as i64);
            ConversationRow {
                id: format!("m_{index}"),
                title: format!("Conversation {index}"),
                teammate: "Wren".into(),
                started_by: "native".into(),
                state: "completed".into(),
                updated: at.to_rfc3339(),
            }
        })
        .collect()
}

fn draw(count: usize, selected_mission_id: Option<&str>) -> Runtime<State> {
    let mut s = State::default();
    s.demo = true;
    s.teammates.push(Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Custom".into(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    });
    if let Some(id) = selected_mission_id {
        let mut conv = Conversation::default();
        conv.conversation_id = Some(id.to_owned());
        s.conversations.insert("tm_wren".into(), conv);
    }
    s.recent = make_rows(count);
    let mut rt = Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (280.0, 6000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn drawn_row_count(rt: &Runtime<State>) -> usize {
    rt.elements()
        .iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("recent-m_")))
        .count()
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements()
        .into_iter()
        .filter_map(|e| e.text)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn draws_one_page_newest_first_and_says_how_many_more_there_are() {
    // case: draws one page, newest first, and says how many more there are
    assert_eq!(LIST_PAGE, 80);
    let mut rt = draw(300, None);
    assert_eq!(drawn_row_count(&rt), 80);
    let said = words(&rt);
    assert!(said.contains("Conversation 0"), "{said}");
    assert!(!said.contains("Conversation 80"), "row at LIST_PAGE must not be drawn yet");
    assert!(said.contains("220 more conversations"), "{said}");

    // Pressing the footer button draws the next page.
    let (x, y) = rt.find("sidebar-list-more").expect("N more conversations button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert_eq!(drawn_row_count(&rt), 160);
    let after = words(&rt);
    assert!(after.contains("Conversation 80"), "second page should now be drawn");
    assert!(after.contains("140 more conversations"), "{after}");
}

#[test]
fn always_draws_far_enough_to_hold_the_one_open() {
    // case: always draws far enough to hold the one open
    let rt = draw(300, Some("m_200"));
    assert_eq!(drawn_row_count(&rt), 201);
    let said = words(&rt);
    assert!(said.contains("Conversation 200"), "{said}");
    assert!(said.contains("99 more conversations"), "{said}");

    // Singular when exactly one remains.
    let rt_one = draw(300, Some("m_298"));
    assert_eq!(drawn_row_count(&rt_one), 299);
    let one_said = words(&rt_one);
    assert!(one_said.contains("1 more conversation"), "{one_said}");
    assert!(!one_said.contains("1 more conversations"), "{one_said}");
}

#[test]
fn is_drawn_whole_when_it_fits_in_a_page_with_nothing_more_to_say() {
    // case: is drawn whole when it fits in a page, with nothing more to say
    let rt = draw(12, None);
    assert_eq!(drawn_row_count(&rt), 12);
    assert!(rt.find("sidebar-list-more").is_none());
    let said = words(&rt);
    assert!(!said.contains("more conversation"), "{said}");
}
