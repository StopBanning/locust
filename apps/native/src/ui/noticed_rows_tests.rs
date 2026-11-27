//! Big Brother's review of 26 Nov (`docs/REVIEW-ui-changes-2026-11-26.md`, change 4), which Colin approved as an
//! improvement on Electron's Home (`ScoutCards.tsx:116-117`, `:157`): "one row per notice: what happened as the title
//! ('A turn hit the 5-hour limit'), its age, then Open and Dismiss together on the right. Fold repeats into one row.
//! Drop a limit notice once the limit has reset." Drawn headless through Home's Scout section.
use super::*;
use rune::{Cx, Env, Runtime};
use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use locust_core::scout::types::{ScoutChip, ScoutDraft, ScoutSettings, ScoutState, ScoutTone};

/// 2026-04-12T13:20:00Z, the clock every notice here is read against.
const NOW_MS: i64 = 1_776_000_000_000;

fn instant(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap()
}

fn iso(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn card(id: &str, urgency: ScoutUrgency, said: &str, hours_old: i64) -> ScoutCard {
    ScoutCard {
        card_id: id.into(),
        at: iso(instant(NOW_MS - hours_old * 3_600_000)),
        reading: false,
        take: None,
        fix: None,
        fact: locust_core::scout::types::ScoutFact::simple(id, urgency, said),
    }
}

/// What Scout writes for a turn that ended on a limit that resets `hours_ahead` hours from the clock.
fn limit_card(id: &str, who: &str, hours_old: i64, hours_ahead: i64) -> ScoutCard {
    let resets = iso(instant(NOW_MS + hours_ahead * 3_600_000));
    let mut c = card(id, ScoutUrgency::Soon, &format!("{who}\u{2019}s turn failed: You\u{2019}ve hit your 5-hour limit \u{b7} resets {resets}"), hours_old);
    c.fact.detail = Some("It was asked: Do the looks pass".into());
    c.fact.chips = Some(vec![ScoutChip::new("failed", ScoutTone::Red)]);
    c.fact.open = Some(ScoutOpen::Url { url: format!("https://example.com/{id}"), label: "Open it".into() });
    c
}

fn state_with(cards: Vec<ScoutCard>) -> State {
    let mut s = State::default();
    s.scout.now_ms = Some(NOW_MS);
    s.scout.state = Some(ScoutState {
        settings: ScoutSettings { on: true, ..ScoutSettings::default() },
        cards,
        look_ids: Vec::new(),
        last_look_at: None,
        model_said: None,
        today: None,
        notes: Vec::new(),
    });
    s
}

fn draw(s: &State, _: &mut Cx<State>) -> El {
    crate::scout_cards::home_section(s, &Look::new(true)).expect("Home's Scout section")
}

fn render(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, draw, (960.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn words(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

fn find(rt: &Runtime<State>, key: &str) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("nothing drawn under {key}: {:?}", words(rt)))
}

fn rows_drawn(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with("scout-card-")).collect()
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("nothing to press under {key}: {:?}", words(rt)));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}

#[test]
fn a_notice_is_one_row_titled_by_what_happened_with_its_age_open_and_dismiss_together_on_the_right() {
    let rt = render(state_with(vec![limit_card("c1", "Wren", 2, 3)]));
    let row = find(&rt, "scout-card-c1");
    let title = find(&rt, "scout-row-title-c1");
    let age = find(&rt, "scout-row-age-c1");
    let open = find(&rt, "scout-open-c1");
    let dismiss = find(&rt, "scout-dismiss-c1");
    assert_eq!(title.text.as_deref(), Some("Wren\u{2019}s turn hit the 5-hour limit"), "{:?}", words(&rt));
    assert_eq!(age.text.as_deref(), Some(locust_core::scout::view::ago(&iso(instant(NOW_MS - 2 * 3_600_000)), NOW_MS).as_str()));
    // A row, not a card: one line tall.
    assert!(row.rect[3] < 56.0, "a row is one line tall, this is {}px", row.rect[3]);
    // One line: everything on the row's own height.
    let centre = |e: &rune::ElementInfo| e.rect[1] + e.rect[3] / 2.0;
    for e in [&title, &age, &open, &dismiss] {
        assert!((centre(e) - centre(&row)).abs() < row.rect[3] / 2.0, "{:?} is not on the row's line", e.key);
    }
    // Title, age, then Open and Dismiss together at the right: Open beside Dismiss (not at the opposite edge), both
    // right of the words.
    assert!(title.rect[0] < age.rect[0] && age.rect[0] < open.rect[0] && open.rect[0] < dismiss.rect[0], "title, age, Open, Dismiss in order");
    let gap = dismiss.rect[0] - (open.rect[0] + open.rect[2]);
    assert!((0.0..=16.0).contains(&gap), "Open and Dismiss sit together, {gap}px apart");
    let right_edge = row.rect[0] + row.rect[2];
    assert!(right_edge - (dismiss.rect[0] + dismiss.rect[2]) <= 16.0, "Dismiss is at the row's right edge");
    assert!(open.rect[0] > row.rect[0] + row.rect[2] / 2.0, "Open is in the right half, not at the left");
}

#[test]
fn the_row_carries_none_of_the_old_card_words() {
    let rt = render(state_with(vec![limit_card("c1", "Wren", 2, 3)]));
    let said = words(&rt);
    assert!(!said.iter().any(|w| w == "Noticed"), "the card's own title repeats the heading: {said:?}");
    assert!(!said.iter().any(|w| w.starts_with("Scout \u{b7}")), "the stamp is gone: {said:?}");
    assert!(!said.iter().any(|w| w.starts_with("It was asked")), "the detail is not a row's: {said:?}");
    assert!(said.iter().any(|w| w == "Scout noticed \u{b7} 1"), "{said:?}");
}

#[test]
fn the_same_limit_noticed_twice_is_one_row_that_dismisses_both() {
    let mut rt = render(state_with(vec![limit_card("c1", "Wren", 2, 3), limit_card("c2", "Atlas", 1, 3)]));
    // The newest of the repeats speaks for the row.
    assert_eq!(rows_drawn(&rt), vec!["scout-card-c2".to_owned()], "one row for the one limit: {:?}", words(&rt));
    assert!(words(&rt).iter().any(|w| w == "Scout noticed \u{b7} 1"), "{:?}", words(&rt));
    assert_eq!(find(&rt, "scout-row-title-c2").text.as_deref(), Some("Turns hit the 5-hour limit"));
    assert_eq!(find(&rt, "scout-row-count-c2").text.as_deref(), Some("2\u{d7}"));
    press(&mut rt, "scout-dismiss-c2");
    assert!(rt.state.scout.state.as_ref().unwrap().cards.is_empty(), "both went: {:?}", rt.state.scout.state.as_ref().unwrap().cards.iter().map(|c| &c.card_id).collect::<Vec<_>>());
}

#[test]
fn two_limits_that_reset_at_different_times_are_two_rows() {
    let rt = render(state_with(vec![limit_card("c1", "Wren", 2, 3), limit_card("c2", "Wren", 1, 30)]));
    assert_eq!(rows_drawn(&rt).len(), 2, "{:?}", words(&rt));
}

#[test]
fn a_notice_that_repeats_word_for_word_is_one_row() {
    let a = card("c1", ScoutUrgency::Soon, "PR #7 failing", 5);
    let b = card("c2", ScoutUrgency::Soon, "PR #7 failing", 1);
    let other = card("c3", ScoutUrgency::Soon, "PR #9 failing", 1);
    let rt = render(state_with(vec![a, b, other]));
    assert_eq!(rows_drawn(&rt).len(), 2, "{:?}", words(&rt));
    assert!(words(&rt).iter().any(|w| w == "Scout noticed \u{b7} 2"), "the count says rows, not cards: {:?}", words(&rt));
    assert_eq!(find(&rt, "scout-row-count-c2").text.as_deref(), Some("2\u{d7}"), "the newest of the two speaks for the row");
}

#[test]
fn a_limit_notice_goes_once_that_limit_has_reset() {
    let over = limit_card("c1", "Wren", 21, -3);
    let live = limit_card("c2", "Atlas", 1, 3);
    let rt = render(state_with(vec![over, live]));
    assert_eq!(rows_drawn(&rt), vec!["scout-card-c2".to_owned()], "{:?}", words(&rt));
    assert!(words(&rt).iter().any(|w| w == "Scout noticed \u{b7} 1"), "{:?}", words(&rt));
}

#[test]
fn when_every_notice_was_a_limit_that_has_reset_home_says_scout_is_watching() {
    let rt = render(state_with(vec![limit_card("c1", "Wren", 21, -3), limit_card("c2", "Wren", 15, -9)]));
    assert!(rows_drawn(&rt).is_empty());
    assert!(words(&rt).iter().any(|w| w.starts_with("Scout is watching")), "{:?}", words(&rt));
}

#[test]
fn a_limit_that_names_no_reset_time_stays_until_it_is_dismissed() {
    let mut c = card("c1", ScoutUrgency::Soon, "Wren\u{2019}s turn failed: You\u{2019}ve hit your 5-hour limit", 30);
    c.fact.open = Some(ScoutOpen::Url { url: "https://example.com/c1".into(), label: "Open it".into() });
    let rt = render(state_with(vec![c]));
    assert_eq!(rows_drawn(&rt), vec!["scout-card-c1".to_owned()]);
    assert_eq!(find(&rt, "scout-row-title-c1").text.as_deref(), Some("Wren\u{2019}s turn hit the 5-hour limit"));
}

#[test]
fn a_notice_that_needs_you_now_keeps_its_whole_card() {
    let mut now = card("c1", ScoutUrgency::Now, "Atlas has waited 25 minutes for your OK.", 1);
    now.fact.draft = Some(ScoutDraft { teammate_id: "tm_atlas".into(), name: "Atlas".into(), message: "Go ahead.".into() });
    let rt = render(state_with(vec![now]));
    assert!(words(&rt).iter().any(|w| w == "Send to Atlas"), "{:?}", words(&rt));
    assert!(rt.find("scout-row-title-c1").is_none(), "a card with a draft cannot be a row");
}

#[test]
fn a_soon_notice_with_a_draft_keeps_its_whole_card() {
    let mut c = card("c1", ScoutUrgency::Soon, "Lin asked about the token table.", 1);
    c.fact.draft = Some(ScoutDraft { teammate_id: "lin".into(), name: "Lin".into(), message: "Use the small one.".into() });
    let rt = render(state_with(vec![c]));
    assert!(rt.find("scout-row-title-c1").is_none());
    assert!(words(&rt).iter().any(|w| w == "Send to Lin"), "{:?}", words(&rt));
}

#[test]
fn more_than_three_rows_still_say_show_all_and_the_count_is_of_rows() {
    let cards: Vec<_> = (1..=5).map(|n| card(&format!("c{n}"), ScoutUrgency::Soon, &format!("Notice number {n}"), 1)).collect();
    let rt = render(state_with(cards));
    assert_eq!(rows_drawn(&rt).len(), 3);
    assert!(words(&rt).iter().any(|w| w == "Scout noticed \u{b7} 5"));
    assert!(words(&rt).iter().any(|w| w == "Show all 5"));
}

#[test]
fn open_on_the_row_opens_what_the_notice_points_at() {
    let mut rt = render(state_with(vec![limit_card("c1", "Wren", 2, 3)]));
    press(&mut rt, "scout-open-c1");
    assert_eq!(rt.state.opened_links, vec!["https://example.com/c1".to_owned()]);
}

// --- the limit's reset, read out of the notice's words ---

fn written() -> DateTime<Utc> {
    instant(NOW_MS)
}

#[test]
fn a_clock_time_is_the_next_time_the_clock_reads_that_after_the_notice_was_written() {
    let target = (written() + Duration::hours(3)).with_timezone(&Local);
    let said = format!("You\u{2019}ve hit your 5-hour limit \u{b7} resets {}", target.format("%-I:%M%P"));
    assert_eq!(reset_of(&said, written()), Some(target.with_timezone(&Utc)), "{said}");
    // One that already passed today is tomorrow's.
    let earlier = (written() - Duration::hours(1)).with_timezone(&Local);
    let said = format!("resets {}", earlier.format("%-I:%M %p"));
    assert_eq!(reset_of(&said, written()), Some((written() - Duration::hours(1) + Duration::hours(24)).with_timezone(&Utc)), "{said}");
}

#[test]
fn a_weekday_and_a_clock_time_is_the_next_such_day() {
    let target = Local.from_local_datetime(&(written() + Duration::days(3)).with_timezone(&Local).date_naive().and_hms_opt(20, 30, 0).unwrap()).earliest().unwrap();
    let said = format!("resets {} 8:30 PM", target.format("%a"));
    assert_eq!(reset_of(&said, written()), Some(target.with_timezone(&Utc)), "{said}");
}

#[test]
fn an_instant_is_read_as_it_is_and_words_with_no_time_give_none() {
    assert_eq!(reset_of("resets 2026-11-26T01:30:00.000Z", written()), DateTime::parse_from_rfc3339("2026-11-26T01:30:00.000Z").ok().map(|t| t.with_timezone(&Utc)));
    assert_eq!(reset_of("You\u{2019}ve hit your limit", written()), None);
}

#[test]
fn only_a_usage_limit_is_read_as_one() {
    let said = |text: &str| card("c", ScoutUrgency::Soon, text, 1);
    assert!(limit_of(&said("Wren\u{2019}s turn failed: You\u{2019}ve hit your limit \u{b7} resets 8pm")).is_some());
    assert!(limit_of(&said("Claude Code reached its 5-hour limit")).is_some());
    assert!(limit_of(&said("Usage limit reached")).is_some());
    assert!(limit_of(&said("Wren\u{2019}s turn failed: the build hit a memory limit")).is_none());
    assert!(limit_of(&said("PR #7 failing")).is_none());
}

#[test]
fn a_row_the_person_was_brought_to_comes_first_even_past_the_three() {
    let cards: Vec<_> = (1..=5).map(|n| card(&format!("c{n}"), ScoutUrgency::Soon, &format!("Notice number {n}"), 1)).collect();
    let mut s = state_with(cards);
    s.scout.focus_card_id = Some("c5".into());
    let rt = render(s);
    let drawn = rows_drawn(&rt);
    assert_eq!(drawn.first().map(String::as_str), Some("scout-card-c5"), "{drawn:?}");
    assert_eq!(drawn.len(), 4, "the three and the one brought to: {drawn:?}");
}

#[test]
fn pressing_dismiss_on_a_single_row_takes_only_that_notice() {
    let a = card("c1", ScoutUrgency::Soon, "PR #7 failing", 1);
    let b = card("c2", ScoutUrgency::Soon, "PR #9 failing", 1);
    let mut rt = render(state_with(vec![a, b]));
    press(&mut rt, "scout-dismiss-c1");
    let left: Vec<_> = rt.state.scout.state.as_ref().unwrap().cards.iter().map(|c| c.card_id.clone()).collect();
    assert_eq!(left, vec!["c2".to_owned()]);
}
