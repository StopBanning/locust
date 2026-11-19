//! settings-search-knows-the-words.test.ts: the four words Grok typed and got nothing for (memory, worktree, node,
//! ledger) find their page and are answered with the heading they lead to; the words that already worked still do; a
//! word on no page answers nothing; and the index files every extra word under a real heading, on every page, once.
use super::*;

/// The four words measured missing on 0.177.0, and where each lives.
const THE_FOUR: [(&str, SettingsPageId, &str); 4] = [
    ("memory", SettingsPageId::Memory, "What your team remembers"),
    ("worktree", SettingsPageId::Workspace, "Project folder"),
    ("node", SettingsPageId::Runtimes, "AI agents & accounts"),
    ("ledger", SettingsPageId::Privacy, "Privacy & local data"),
];

/// The pages that answer a typed word, the way the screen filters them.
fn answering(typed: &str) -> Vec<SettingsPageId> {
    SETTINGS_PAGES.iter().filter(|p| page_matches(p, typed)).map(|p| p.id).collect()
}

#[test]
fn typed_finds_the_page_page() {
    for (typed, page, _) in THE_FOUR {
        assert!(answering(typed).contains(&page), "{typed} -> {}", page.as_str());
    }
}

#[test]
fn typed_is_answered_with_where_it_lives_not_with_itself() {
    for (typed, page, heading) in THE_FOUR {
        assert!(matched_headings(page_by_id(page), typed).contains(&heading), "{typed} -> {heading}");
    }
}

#[test]
fn typed_does_not_drag_in_every_page() {
    for (typed, _, _) in THE_FOUR {
        assert!(answering(typed).len() < SETTINGS_PAGES.len(), "{typed} matched every page");
    }
}

#[test]
fn typed_still_finds_heading() {
    for (typed, page, heading) in [
        ("trash", SettingsPageId::Privacy, "Trash"),
        ("connector", SettingsPageId::Connectors, "Connectors"),
        ("update", SettingsPageId::App, "Updates"),
        ("auto", SettingsPageId::Teammates, "Auto mode"),
    ] {
        assert!(answering(typed).contains(&page), "{typed} -> {}", page.as_str());
        assert!(matched_headings(page_by_id(page), typed).contains(&heading), "{typed} -> {heading}");
    }
}

#[test]
fn a_word_that_is_on_no_page_still_answers_honestly() {
    assert!(answering("kombucha").is_empty());
}

#[test]
fn an_empty_search_is_not_a_match_for_everything_by_accident() {
    assert_eq!(answering("").len(), SETTINGS_PAGES.len());
}

#[test]
fn files_every_extra_word_under_a_heading_the_page_really_has() {
    for page in SETTINGS_PAGES {
        for (heading, _) in page.also_known_as {
            assert!(page.headings.contains(heading), "{} -> {heading}", page.id.as_str());
        }
    }
}

#[test]
fn has_extra_words_at_all_on_every_page() {
    for page in SETTINGS_PAGES {
        assert!(!page.also_known_as.is_empty(), "{}", page.id.as_str());
    }
}

#[test]
fn never_repeats_a_word_across_two_pages_which_would_make_the_search_useless() {
    let mut seen = std::collections::HashMap::new();
    for page in SETTINGS_PAGES {
        for (_, words) in page.also_known_as {
            for word in *words {
                if let Some(other) = seen.insert(*word, page.id.as_str()) {
                    panic!("{word:?} is on {} and {other}", page.id.as_str());
                }
            }
        }
    }
}
