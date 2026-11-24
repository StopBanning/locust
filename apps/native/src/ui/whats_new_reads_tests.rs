//! Electron's `whats-new-reads-like-claude-codes.test.tsx`, the cases that draw: one build (its date, its version as a
//! badge, its groups under their labels), Settings' What's new (a page of builds, newest first, then Show older
//! versions; reading, or shipped without), and the splash after a big update -- all drawn by the window itself.
use super::*;
use rune::{Env, Runtime};

fn window(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, crate::view, (1200.0, 6000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.0);
    rt
}

/// Settings open on What's new, holding these notes (or none read yet).
fn whats_new(notes: Option<&str>) -> Runtime<State> {
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::WhatsNew);
    if let Some(notes) = notes {
        s.home.changelog = changelog::entries(notes);
        s.home.changelog_read = true;
        s.home.changelog_shown = changelog::WHATS_NEW_PAGE;
    }
    window(s)
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

fn keyed(rt: &Runtime<State>, key: &str) -> Option<rune::ElementInfo> {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key))
}

#[test]
fn says_its_date_wears_its_version_as_a_badge_and_groups_its_changes_under_their_labels() {
    // case: says its date, wears its version as a badge, and groups its changes under their labels
    let rt = whats_new(Some("## 0.281.0 - 2026-09-23\n\n### New\n\n- **What's new, in Settings.** Every version.\n\n### Improved\n\n- **No banner.** Gone.\n"));
    let date = keyed(&rt, "release-date-0.281.0").expect("the date heads the build");
    assert_eq!(date.text.as_deref(), Some("September 23, 2026"));
    let badge = keyed(&rt, "release-version-0.281.0").expect("the version is drawn on its own");
    assert_eq!(badge.text.as_deref(), Some("0.281.0"));
    assert!(badge.radius > 0.0, "a badge, rounded");
    assert!(badge.rect[0] > date.rect[0], "beside the date");
    assert_eq!(keyed(&rt, "release-label-0.281.0-0").and_then(|e| e.text).as_deref(), Some("New"));
    assert_eq!(keyed(&rt, "release-label-0.281.0-1").and_then(|e| e.text).as_deref(), Some("Improved"));
    let said = words(&rt);
    assert!(said.contains("What's new, in Settings."), "{said}");
    assert!(!said.contains("**What"), "the bold is drawn, not its asterisks: {said}");
}

#[test]
fn written_before_the_groups_carries_no_label() {
    // case: written before the groups, carries no label
    let rt = whats_new(Some("## 0.100.0 - 2026-09-01\n\n- An old change.\n"));
    assert!(rt.elements().iter().all(|e| !e.key.as_deref().is_some_and(|k| k.starts_with("release-label-"))));
    assert!(words(&rt).contains("An old change."));
}

#[test]
fn draws_the_newest_builds_first_and_offers_the_older_ones_rather_than_drawing_all_of_them_at_once() {
    // case: draws the newest builds first, and offers the older ones rather than drawing all of them at once
    let notes: String = (0..12 + 5).map(|i| format!("## 0.{}.0 - 2026-09-23\n\n- Change {i}.\n\n", 281 - i)).collect();
    let rt = whats_new(Some(&notes));
    let drawn = rt.elements().into_iter().filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.starts_with("release-date-"))).count();
    assert_eq!(drawn, 12);
    let newest = keyed(&rt, "release-date-0.281.0").expect("0.281.0 is drawn").rect[1];
    let next = keyed(&rt, "release-date-0.280.0").expect("0.280.0 is drawn").rect[1];
    assert!(newest < next, "newest first");
    assert!(keyed(&rt, "release-date-0.269.0").is_none(), "the thirteenth waits");
    assert!(words(&rt).contains("Show older versions"));
}

#[test]
fn says_so_when_the_build_shipped_without_its_changelog_and_while_it_is_being_read() {
    // case: says so when the build shipped without its changelog, and while it is being read
    let shipped_without = words(&whats_new(Some("# Changelog\n")));
    assert!(shipped_without.contains("without its changelog"), "{shipped_without}");
    assert!(!shipped_without.contains("Reading what changed"), "{shipped_without}");
    let reading = words(&whats_new(None));
    assert!(reading.contains("Reading what changed"), "{reading}");
    assert!(!reading.contains("without its changelog"), "{reading}");
}

#[test]
fn shows_the_big_builds_and_the_way_to_every_other_one() {
    // case: shows the big builds, and the way to every other one
    let mut s = State::default();
    home_scene(&mut s);
    s.home.splash_entries = changelog::entries("## 0.277.0 - 2026-09-22\n<!-- big -->\n\n- **Every teammate is a bot.**\n");
    s.home.splash_open = true;
    let rt = window(s);
    let said = words(&rt);
    for words in ["What’s new in Locust", "Every teammate is a bot", "September 22, 2026", "See every version", "Got it"] {
        assert!(said.contains(words), "{words} not in: {said}");
    }
    let tree = rt.accessibility_json().to_string();
    assert!(tree.contains("\"dialog\""), "the splash is a dialog: {tree}");
}
