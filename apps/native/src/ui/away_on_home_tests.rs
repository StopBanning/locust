//! Electron's `since-you-were-away-on-home.test.tsx`, drawn by the window: Home's list of what ended while nobody was
//! here heads with the counts and Got it, then names what failed, the routine slots missed and what ran, in that
//! order, each saying who and when; six of a kind, and the rest counted.
use super::*;
use locust_core::away::{AwayItem, AwayMissed};
use rune::{Env, Runtime};

fn item(mission_id: &str, title: &str, at: &str, owner: Option<&str>) -> AwayItem {
    AwayItem { mission_id: mission_id.into(), title: title.into(), at: at.into(), owner_id: owner.map(Into::into), routine_id: None }
}

/// The away list's words top to bottom, and the ids of its run rows, as Home draws them.
fn home_with(summary: AwaySummary) -> (Vec<String>, Vec<String>) {
    let mut s = State::default();
    home_scene(&mut s);
    s.home.splash_open = false;
    s.home.splash_entries.clear();
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
    s.home.away = Some(summary);
    let mut rt = Runtime::new(s, crate::view, (1200.0, 4000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.0);
    let all = rt.elements();
    let boxed = all.iter().find(|e| !e.exiting && e.key.as_deref() == Some("home-away")).expect("the away list is drawn").rect;
    let inside = |r: [f32; 4]| r[1] >= boxed[1] && r[1] + r[3] <= boxed[1] + boxed[3] + 0.5 && r[0] >= boxed[0] - 0.5;
    let mut said: Vec<(f32, f32, String)> = all.iter().filter(|e| !e.exiting && inside(e.rect)).filter_map(|e| e.text.clone().map(|t| (e.rect[1], e.rect[0], t))).collect();
    said.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut rows: Vec<(f32, String)> = all.iter().filter(|e| !e.exiting).filter_map(|e| e.key.as_deref().and_then(|k| k.strip_prefix("away-row-")).map(|id| (e.rect[1], id.to_owned()))).collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.dedup_by(|a, b| a.1 == b.1);
    (said.into_iter().map(|(_, _, t)| t).collect(), rows.into_iter().map(|(_, id)| id).collect())
}

#[test]
fn heads_with_the_counts_and_got_it_then_failed_missed_and_ran_in_that_order_each_naming_who_and_when() {
    // case: heads with the counts and Got it, then failed, missed and ran, in that order, each naming who and when
    let (said, rows) = home_with(AwaySummary {
        since: "2026-10-03T22:00:00.000Z".into(),
        ran: vec![item("m1", "Tidy the build folder", "2026-10-04T05:00:00.000Z", Some("tm_wren")), item("m2", "Write the digest", "2026-10-04T06:00:00.000Z", None)],
        failed: vec![item("m3", "Run the nightly tests", "2026-10-04T02:00:00.000Z", Some("tm_wren"))],
        missed: vec![AwayMissed { routine_id: "rt_2".into(), name: "Morning digest".into(), due_at: "2026-10-04T07:00:00.000Z".into() }],
        waiting: 2,
    });
    let all = said.join("\n");
    assert!(all.contains("Since you were away"), "{all}");
    assert!(all.contains("2 ran · 1 failed · 1 missed · 2 waiting on you"), "{all}");
    assert!(said.iter().any(|t| t == "Got it"), "{all}");
    assert!(all.contains("2 conversations are waiting on you"), "{all}");
    // Failed, then missed, then ran, each kind in the order the summary gives it.
    let at = |words: &str| said.iter().position(|t| t.starts_with(words)).unwrap_or_else(|| panic!("{words} not in: {all}"));
    let order = [at("Run the nightly tests"), at("Morning digest"), at("Tidy the build folder"), at("Write the digest")];
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}: {all}");
    assert_eq!(rows, ["m3", "m1", "m2"]);
    assert!(said[order[0]].contains("failed"), "{all}");
    assert!(said[order[1]].contains("missed, due"), "{all}");
    assert!(said[order[2]].contains("finished") && said[order[3]].contains("finished"), "{all}");
    assert!(said[order[0]].contains("Wren") && said[order[2]].contains("Wren"), "{all}");
    assert!(!said[order[3]].contains("Wren"), "nobody is named for a run nobody owns: {all}");
    assert!(!all.contains("and "), "{all}");
}

#[test]
fn shows_six_of_a_kind_and_counts_the_rest_and_says_nothing_about_waiting_when_nobody_is_control() {
    // case: shows six of a kind and counts the rest, and says nothing about waiting when nobody is (control)
    let ran = (0..9).map(|i| item(&format!("m{i}"), &format!("Turn {i}"), &format!("2026-10-04T0{i}:00:00.000Z"), None)).collect();
    let (said, rows) = home_with(AwaySummary { since: "2026-10-03T22:00:00.000Z".into(), ran, failed: vec![], missed: vec![], waiting: 0 });
    let all = said.join("\n");
    assert!(all.contains("and 3 more in the sidebar"), "{all}");
    assert!(!all.contains("waiting on you"), "{all}");
    assert_eq!(rows.len(), 6, "{rows:?}");
}
