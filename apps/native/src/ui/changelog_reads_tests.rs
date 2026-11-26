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
    s.home.changelog_read = true;
    let mut rt = Runtime::new(s, crate::view, (1200.0, 3000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

/// A build's heading is its date (WhatsNew.tsx `ReleaseNotes`), or "Version x" when it has none.
fn is_heading(w: &str) -> bool {
    w.starts_with("Version ") || w == "September 17, 2026" || w == "January 1, 2026"
}

/// The words drawn under the build whose version badge reads `version`, up to the next build's heading (or the end of
/// the page's notes).
fn under_heading(words: &[String], version: &str) -> Option<String> {
    let at = words.iter().position(|w| w == version)?;
    let rest = &words[at + 1..];
    let end = rest.iter().position(|w| is_heading(w)).unwrap_or(rest.len());
    Some(rest[..end].join("\n"))
}

// case: finds every entry, newest first, in the order the file writes them
#[test]
fn whats_new_lists_every_entry_newest_first_in_file_order() {
    let words = whats_new_words(FILE);
    let headings: Vec<&str> = words.iter().map(String::as_str).filter(|w| is_heading(w) || w.starts_with("0.")).collect();
    assert_eq!(
        headings,
        ["September 17, 2026", "0.176.0", "September 17, 2026", "0.175.0", "Version 0.9.2", "0.9.2"],
        "{words:?}"
    );
    let versions: Vec<String> = changelog::entries(FILE).into_iter().map(|e| e.version).collect();
    assert_eq!(versions, ["0.176.0", "0.175.0", "0.9.2"]);
}

// case: gives an entry its body and not its heading
#[test]
fn an_entry_shows_its_body_under_its_heading_and_not_the_next_ones() {
    let words = whats_new_words(FILE);
    let body = under_heading(&words, "0.175.0").expect("0.175.0 is drawn");
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
    let body = under_heading(&words, "0.176.0").expect("0.176.0 is drawn");
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
    let body = under_heading(&words, "0.9.2").expect("0.9.2 is drawn by its version alone");
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
    assert!(!words.iter().any(|w| w == "Version 2.0.0" || w == "2.0.0"), "{words:?}");
    let body = under_heading(&words, "1.0.0").expect("1.0.0 is drawn");
    assert!(body.contains("Mentions 2.0.0"), "{body}");
    // A heading starts its line: a line that only ends in a version after `##` is still the entry's words.
    let inside = "## 1.0.0 - 2026-01-01\n\nSee ## 2.0.0\n";
    let versions: Vec<String> = changelog::entries(inside).into_iter().map(|e| e.version).collect();
    assert_eq!(versions, ["1.0.0"]);
    assert!(changelog::entry_for(inside, "1.0.0").expect("1.0.0").body.contains("See ## 2.0.0"));
}

/// `refresh_changelog` on a fresh window, as opening Home runs it, then Settings > What's new drawn: every word.
fn after_refresh(seen: Option<&str>) -> (Panel, Vec<String>) {
    let mut s = State::default();
    s.home.seen_version = seen.map(str::to_owned);
    refresh_changelog(&mut s);
    let home = Panel {
        current_version: s.home.current_version.clone(),
        changelog: s.home.changelog.clone(),
        splash_entries: s.home.splash_entries.clone(),
        splash_open: s.home.splash_open,
        ..Panel::default()
    };
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::WhatsNew);
    let mut rt = Runtime::new(s, crate::view, (1200.0, 3000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let words = rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect();
    (home, words)
}

// case: main/index.ts readChangelogOnce: `const version = app.getVersion()`, never the changelog's newest heading
#[test]
fn the_version_the_window_says_is_the_apps_own_not_the_changelogs_newest_heading() {
    // Electron's app.getVersion() is the package's own version; the crate's is CARGO_PKG_VERSION.
    let own = env!("CARGO_PKG_VERSION");
    let (home, words) = after_refresh(None);
    assert_eq!(home.current_version, own);
    let newest = home.changelog.first().map(|e| e.version.clone());
    assert_ne!(Some(home.current_version.clone()), newest.filter(|v| v != own), "the newest heading is not the running build");
    // Screens.tsx, Settings > What's new: "Every version of Locust, newest first. This is {changelog?.version}."
    let lede = format!("Every version of Locust, newest first. This is {own}.");
    assert!(words.iter().any(|w| *w == lede), "{words:?}");
    assert!(!words.iter().any(|w| w == "Every version of Locust, newest first. This is 0.1.0."), "{words:?}");
}

// case: main/index.ts readChangelogOnce with no entry for the running version: `entries: all` is every entry still,
// `splashEntries` finds no `from` and returns [] (no nearest entry put in its place), and the version stays the app's.
#[test]
fn a_running_build_the_changelog_does_not_carry_still_lists_every_entry_and_shows_no_splash() {
    let own = env!("CARGO_PKG_VERSION");
    let (home, words) = after_refresh(Some("0.0.1"));
    assert!(home.changelog.iter().all(|e| e.version != own), "this test is about a build the notes do not carry");
    assert!(!home.changelog.is_empty(), "the notes were read");
    assert!(home.changelog.iter().any(|e| e.big), "a big entry exists, so a splash would have something to show");
    assert_eq!(home.current_version, own);
    assert!(home.splash_entries.is_empty(), "{:?}", home.splash_entries);
    assert!(!home.splash_open);
    // What's new lists every entry the file carries, by its version badge.
    for entry in &home.changelog {
        assert!(words.iter().any(|w| *w == entry.version), "{} missing from {words:?}", entry.version);
    }
    assert!(!words.iter().any(|w| w == "This build shipped without its changelog."), "{words:?}");
    // main/changelog.ts splashEntries: `if (from < 0 || to <= from) return []`. A newer big entry and a seen one both
    // in the file still show nothing when the running build is not in it: no nearest entry stands in for it.
    let notes = "## 0.2.0 - 2026-11-20\n<!-- big -->\n\n- **Big.**\n\n## 0.1.0 - 2026-11-10\n\n- Small.\n";
    let all = changelog::entries(notes);
    assert_eq!(changelog::splash_entries(&all, Some("0.1.0"), "0.2.0").len(), 1, "the file does mark 0.2.0 big");
    assert!(changelog::splash_entries(&all, Some("0.1.0"), own).is_empty());
}
