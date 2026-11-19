//! settings-pages-are-real.test.ts: the search index is held to the screen. Every page drawn headless with nothing
//! typed; every section heading it draws (role "heading", level 2, `section_heading` and its kin) is in the index, and
//! nothing is in the index that no page draws. A conditional heading is drawn under its condition: "Managed by your
//! organization" with a policy in place.
use super::*;
use crate::settings_pages::{SETTINGS_PAGES, SettingsPageId};
use rune::{Env, Runtime};

struct PolicyGuard;
impl Drop for PolicyGuard {
    fn drop(&mut self) {
        locust_core::org_policy::use_policy_on_this_thread(None);
    }
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    super::view(s, &Palette::new(true))
}

/// The page with this id, drawn tall enough that nothing scrolls out: its title and its section headings.
fn drawn(id: SettingsPageId) -> (Vec<String>, Vec<String>) {
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(id);
    s.settings_panel.phone = Some(locust_core::phone_host::PhoneSettings::default());
    let mut rt = Runtime::new(s, view, (1200.0, 6000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let headings = all
        .iter()
        .filter(|e| e.role == "heading" && e.level == Some(2) && !e.exiting)
        .filter_map(|e| e.text.clone())
        .collect();
    (all.into_iter().filter_map(|e| e.text).collect(), headings)
}

/// Every page, drawn under a policy so the organization's heading is there.
fn every_page() -> Vec<(SettingsPageId, Vec<String>, Vec<String>)> {
    // On this thread only: the other Settings tests in this process draw with no policy at the same time.
    let _guard = PolicyGuard;
    locust_core::org_policy::use_policy_on_this_thread(Some(crate::managed_controls_tests::sample_policy()));
    SETTINGS_PAGES
        .iter()
        .map(|page| {
            let (texts, headings) = drawn(page.id);
            (page.id, texts, headings)
        })
        .collect()
}

fn indexed() -> Vec<String> {
    SETTINGS_PAGES.iter().flat_map(|p| p.headings.iter().map(|h| (*h).to_owned())).collect()
}

#[test]
fn are_found_at_all_so_this_cannot_pass_by_reading_nothing() {
    let rendered: usize = every_page().iter().map(|(_, _, headings)| headings.len()).sum();
    assert!(rendered > 10, "{rendered} headings drawn");
    assert!(indexed().len() > 10);
    assert!(SETTINGS_PAGES.len() > 3);
}

#[test]
fn index_every_heading_the_screen_renders_and_invent_none() {
    let mut wrong = Vec::new();
    for (id, _, headings) in every_page() {
        let page = crate::settings_pages::page_by_id(id);
        let mut drawn: Vec<String> = headings;
        drawn.sort();
        drawn.dedup();
        let mut listed: Vec<String> = page.headings.iter().map(|h| (*h).to_owned()).collect();
        listed.sort();
        if drawn != listed {
            wrong.push(format!("page {}: drawn {drawn:?}, indexed {listed:?}", id.as_str()));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn are_the_areas_the_screen_actually_splits_on() {
    for (id, texts, headings) in every_page() {
        let page = crate::settings_pages::page_by_id(id);
        // The page's own name heads it, so a page in the index with nothing behind it would show as someone else's.
        assert!(texts.iter().any(|t| t == page.label), "page {} draws its name", id.as_str());
        assert!(!headings.is_empty(), "page {} draws its own sections", id.as_str());
        for other in SETTINGS_PAGES.iter().filter(|p| p.id != id) {
            for heading in other.headings.iter().filter(|h| !page.headings.contains(h)) {
                assert!(!headings.iter().any(|h| h == heading), "{heading:?} of {} drawn on {}", other.id.as_str(), id.as_str());
            }
        }
    }
    let ids: std::collections::HashSet<_> = SETTINGS_PAGES.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids.len(), SETTINGS_PAGES.len(), "two pages with one id would make one unreachable");
}

#[test]
fn a_page_says_its_own_name_once_besides_the_nav() {
    let mut twice = Vec::new();
    for (id, texts, _) in every_page() {
        let page = crate::settings_pages::page_by_id(id);
        let said = texts.iter().filter(|t| *t == page.label).count();
        // One in the nav, one heading the page.
        if said != 2 {
            twice.push(format!("{} said {said} times", page.label));
        }
    }
    assert!(twice.is_empty(), "{}", twice.join("\n"));
}
