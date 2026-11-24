//! Electron's `main/changelog.test.ts` through what reads the file: Settings > What's new draws `changelog::entries`
//! (`whats_new_settings`), the splash picks from them, and `refresh_changelog` asks `entry_for` for the running build.
//! The file and every expected word are Electron's, written out.
use super::*;
use rune::{Env, Runtime};

const FILE: &str = "# Changelog\n\nWhat changed in each build, written for someone using Locust.\n\n## 0.176.0 - 2026-09-17\n\n- **Settings is a list of pages.** Not one long scroll.\n- Search finds a setting by its own name.\n\n## 0.175.0 - 2026-09-17\n\n- **Deleting a conversation can be undone.**\n\n## 0.9.2\n\n- The first one, with no date on it.\n";

/// Settings > What's new with `file` as the notes, as `settings_panel::open` loads them: every drawn word, in order.
fn whats_new_words(file: &str) -> Vec<String> {
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::WhatsNew);
    s.home.changelog = changelog::entries(file);
    s.home.changelog_shown = changelog::WHATS_NEW_PAGE;
    let mut rt = Runtime::new(s, crate::view, (1200.0, 3000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

/// The words drawn under the heading `heading`, up to the next version's heading (or the end of the page's notes).
fn under_heading(words: &[String], heading: &str) -> Option<String> {
    let at = words.iter().position(|w| w == heading)?;
    let rest = &words[at + 1..];
    let end = rest.iter().position(|w| w.starts_with("Version ") || w.contains(" · ")).unwrap_or(rest.len());
    Some(rest[..end].join("\n"))
}

// case: finds every entry, newest first, in the order the file writes them
#[test]
fn whats_new_lists_every_entry_newest_first_in_file_order() {
    let words = whats_new_words(FILE);
    let headings: Vec<&str> = words.iter().map(String::as_str).filter(|w| w.starts_with("Version ") || w.starts_with("0.")).collect();
    assert_eq!(headings, ["0.176.0 · September 17, 2026", "0.175.0 · September 17, 2026", "Version 0.9.2"], "{words:?}");
    let versions: Vec<String> = changelog::entries(FILE).into_iter().map(|e| e.version).collect();
    assert_eq!(versions, ["0.176.0", "0.175.0", "0.9.2"]);
}

// case: gives an entry its body and not its heading
#[test]
fn an_entry_shows_its_body_under_its_heading_and_not_the_next_ones() {
    let words = whats_new_words(FILE);
    let body = under_heading(&words, "0.175.0 · September 17, 2026").expect("0.175.0 is drawn");
    assert!(body.contains("Deleting a conversation can be undone."), "{body}");
    assert!(!body.contains("##"), "{body}");
    // And not the next entry's text, which is the mistake a greedy match makes.
    assert!(!body.contains("first one"), "{body}");
    let entry = changelog::entry_for(FILE, "0.175.0").expect("the file carries 0.175.0");
    assert_eq!(entry.date.as_deref(), Some("2026-09-17"));
    assert_eq!(entry.body, "- **Deleting a conversation can be undone.**");
}

// case: keeps a multi-line entry whole
#[test]
fn a_multi_line_entry_is_drawn_whole() {
    let words = whats_new_words(FILE);
    let body = under_heading(&words, "0.176.0 · September 17, 2026").expect("0.176.0 is drawn");
    assert!(body.contains("Not one long scroll."), "{body}");
    assert!(body.contains("Search finds a setting by its own name."), "{body}");
    let entry = changelog::entry_for(FILE, "0.176.0").expect("the file carries 0.176.0");
    assert_eq!(
        entry.body.split('\n').collect::<Vec<_>>(),
        ["- **Settings is a list of pages.** Not one long scroll.", "- Search finds a setting by its own name."]
    );
}

// case: reads an entry with no date
#[test]
fn an_entry_with_no_date_is_drawn_by_its_version() {
    let words = whats_new_words(FILE);
    let body = under_heading(&words, "Version 0.9.2").expect("0.9.2 is drawn by its version alone");
    assert!(body.contains("The first one, with no date on it."), "{body}");
    let entry = changelog::entry_for(FILE, "0.9.2").expect("the file carries 0.9.2");
    assert_eq!((entry.version.as_str(), entry.body.as_str()), ("0.9.2", "- The first one, with no date on it."));
    assert_eq!(entry.date, None);
}

// case: says nothing for a version the file does not carry
#[test]
fn a_build_the_file_does_not_carry_gets_no_entry_and_no_splash() {
    // A build newer than the changelog is a build whose entry was not written.
    assert_eq!(changelog::entry_for(FILE, "0.177.0"), None);
    assert_eq!(changelog::entry_for(FILE, ""), None);
    // The splash, the window's reader of the running build, says nothing rather than the nearest entry.
    let all = changelog::entries(FILE);
    assert!(changelog::splash_entries(&all, Some("0.9.2"), "0.177.0").is_empty());
}

// case: reads an empty or headingless file as no entries at all
#[test]
fn an_empty_or_headingless_file_draws_the_empty_line() {
    assert_eq!(changelog::entries(""), vec![]);
    assert_eq!(changelog::entries("# Changelog\n\nNothing yet."), vec![]);
    for file in ["", "# Changelog\n\nNothing yet."] {
        let words = whats_new_words(file);
        assert!(words.iter().any(|w| w == "This build shipped without its changelog."), "{words:?}");
        assert!(!words.iter().any(|w| w.contains("Nothing yet.")), "{words:?}");
    }
}

// case: is not fooled by a version-looking line inside an entry
#[test]
fn a_version_inside_an_entry_is_not_a_heading() {
    let tricky = "## 1.0.0 - 2026-01-01\n\nMentions 2.0.0 in passing, and ## not a heading here.\n";
    let versions: Vec<String> = changelog::entries(tricky).into_iter().map(|e| e.version).collect();
    assert_eq!(versions, ["1.0.0"]);
    assert!(changelog::entry_for(tricky, "1.0.0").expect("1.0.0").body.contains("Mentions 2.0.0"));
    let words = whats_new_words(tricky);
    assert!(!words.iter().any(|w| w == "Version 2.0.0" || w.starts_with("2.0.0")), "{words:?}");
    let body = under_heading(&words, "1.0.0 · January 1, 2026").expect("1.0.0 is drawn");
    assert!(body.contains("Mentions 2.0.0"), "{body}");
    // A heading starts its line: a line that only ends in a version after `##` is still the entry's words.
    let inside = "## 1.0.0 - 2026-01-01\n\nSee ## 2.0.0\n";
    let versions: Vec<String> = changelog::entries(inside).into_iter().map(|e| e.version).collect();
    assert_eq!(versions, ["1.0.0"]);
    assert!(changelog::entry_for(inside, "1.0.0").expect("1.0.0").body.contains("See ## 2.0.0"));
}
