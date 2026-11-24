//! Electron's renderer `a-conversation-can-be-put-away.test.tsx` through the window: the sidebar as drawn (`sidebar`,
//! which leaves out what `org.settled` holds while `settled::is_put_away` says so) and the conversation's right-click
//! menu (`context_menu::conversation_menu_at`, whose Snooze list is `settled::snooze_choices` from the time it opened).
//! Expected words and times are Electron's, written out.
use super::*;
use crate::settled::SettledEntry;
use chrono::TimeZone;

fn iso(at: chrono::DateTime<chrono::Utc>) -> String {
    at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// The sidebar's words over Alpha and Bravo, each last active at `last_at`, with Bravo put away as `bravo`, and
/// `open` the conversation on screen.
fn sidebar_words(last_at: &str, bravo: Option<SettledEntry>, open: Option<&str>) -> Vec<String> {
    let mut s = State::default();
    s.demo = true;
    for (id, title) in [("m_a", "Alpha"), ("m_b", "Bravo")] {
        s.recent.push(ConversationRow { id: id.into(), title: title.into(), teammate: String::new(), started_by: "native".into(), state: "completed".into(), updated: last_at.into() });
    }
    if let Some(entry) = bravo {
        s.org.settled.insert("m_b".into(), entry);
    }
    if let Some(id) = open {
        s.teammates.push(Teammate { id: "tm".into(), name: "Wren".into(), role: String::new(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: None });
        s.conversations.insert("tm".into(), Conversation { conversation_id: Some(id.into()), turns: vec![] });
    }
    let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (crate::window_glass::SIDEBAR_WIDTH, 1400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

fn lists(words: &[String], title: &str) -> bool {
    words.iter().any(|w| w == title)
}

// case: is out of the list while settled, or until its snooze ends
#[test]
fn a_settled_or_snoozed_conversation_is_out_of_the_list_until_its_snooze_ends() {
    let now = chrono::Utc::now();
    let (last, at) = (iso(now - chrono::Duration::hours(2)), iso(now - chrono::Duration::hours(1)));
    let settled = SettledEntry { at: at.clone(), until: None };
    assert!(!lists(&sidebar_words(&last, Some(settled), None), "Bravo"));
    let snoozed = SettledEntry { at: at.clone(), until: Some(iso(now + chrono::Duration::hours(5))) };
    assert!(!lists(&sidebar_words(&last, Some(snoozed), None), "Bravo"));
    let snooze_over = SettledEntry { at, until: Some(iso(now - chrono::Duration::seconds(1))) };
    assert!(lists(&sidebar_words(&last, Some(snooze_over), None), "Bravo"));
    assert!(lists(&sidebar_words(&last, None, None), "Bravo"));
}

// case: comes back when something new happens in it
#[test]
fn a_settled_conversation_comes_back_when_something_new_happens() {
    let now = chrono::Utc::now();
    let settled = SettledEntry { at: iso(now - chrono::Duration::hours(1)), until: None };
    let words = sidebar_words(&iso(now - chrono::Duration::minutes(30)), Some(settled), None);
    assert!(lists(&words, "Bravo"), "{words:?}");
}

/// The Snooze list of Bravo's right-click menu opened at `now`: (label, when it ends, in ms).
fn snooze_offered(now: chrono::DateTime<chrono::Local>) -> Vec<(String, i64)> {
    let mut s = State::default();
    s.recent.push(ConversationRow { id: "m_b".into(), title: "Bravo".into(), teammate: String::new(), started_by: "native".into(), state: "completed".into(), updated: "2026-10-10T05:00:00.000Z".into() });
    let menu = crate::context_menu::conversation_menu_at(&s, "m_b", "Bravo", (0.0, 0.0), now);
    let snooze = menu.items.into_iter().find(|item| item.label == "Snooze").expect("Snooze is offered");
    snooze
        .submenu
        .unwrap_or_default()
        .into_iter()
        .map(|item| {
            let until = item.action.strip_prefix("snooze:").expect("a snooze").to_owned();
            (item.label, chrono::DateTime::parse_from_rfc3339(&until).unwrap().timestamp_millis())
        })
        .collect()
}

fn local(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> chrono::DateTime<chrono::Local> {
    chrono::Local.with_ymd_and_hms(year, month, day, hour, minute, 0).single().unwrap()
}

// case: on a Friday morning: an hour, this evening, tomorrow, Monday
#[test]
fn on_a_friday_morning_the_menu_offers_an_hour_this_evening_tomorrow_and_monday() {
    let friday = local(2026, 10, 9, 10, 30);
    assert_eq!(
        snooze_offered(friday),
        vec![
            ("For an hour".to_owned(), local(2026, 10, 9, 11, 30).timestamp_millis()),
            ("Until this evening (6 PM)".to_owned(), local(2026, 10, 9, 18, 0).timestamp_millis()),
            ("Until tomorrow (9 AM)".to_owned(), local(2026, 10, 10, 9, 0).timestamp_millis()),
            ("Until Monday (9 AM)".to_owned(), local(2026, 10, 12, 9, 0).timestamp_millis()),
        ]
    );
}

// case: leaves out an evening already here, and a Monday that is tomorrow
#[test]
fn on_a_sunday_night_the_menu_leaves_out_the_evening_and_monday() {
    let sunday_night = local(2026, 10, 11, 20, 0);
    let labels: Vec<String> = snooze_offered(sunday_night).into_iter().map(|(label, _)| label).collect();
    assert_eq!(labels, ["For an hour", "Until tomorrow (9 AM)"]);
}

// case: leaves a settled conversation out
#[test]
fn the_sidebar_leaves_a_settled_conversation_out() {
    let words = sidebar_words("2026-10-10T05:00:00.000Z", Some(SettledEntry { at: "2026-10-10T06:00:00.000Z".into(), until: None }), None);
    assert!(lists(&words, "Alpha"), "{words:?}");
    assert!(!lists(&words, "Bravo"), "{words:?}");
}

// case: keeps it while it is the one open, so its menu can bring it back
#[test]
fn the_sidebar_keeps_a_settled_conversation_while_it_is_open() {
    let words = sidebar_words("2026-10-10T05:00:00.000Z", Some(SettledEntry { at: "2026-10-10T06:00:00.000Z".into(), until: None }), Some("m_b"));
    assert!(lists(&words, "Bravo"), "{words:?}");
}
