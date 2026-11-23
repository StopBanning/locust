//! ask-for-a-review-is-one-row.test.ts (2 cases), case for case. Colin, 2026-10-19: "ask for a review should prob be
//! folded into a dropdown". Several reviewers open beside one row; one alone says who; none is no row. Choosing a row
//! hands back what it asks for, as Electron's `onSelect` calls `ask` or `askTwo`.
use super::*;

struct T {
    name: &'static str,
    agent: &'static str,
}

impl Named for T {
    fn name(&self) -> &str {
        self.name
    }
}

const BRO: T = T { name: "Bro", agent: "Claude Code" };
const SCOUT: T = T { name: "Scout", agent: "OpenCode" };
const CASPER: T = T { name: "Casper", agent: "Codex CLI" };

fn item(reviewers: &[T], pair: Option<(&T, &T)>) -> Option<MenuRow<String>> {
    review_menu_item(reviewers, pair, |one| one.agent.to_owned(), |one| one.name.to_owned(), |a, b| format!("{}+{}", a.name, b.name))
}

#[test]
fn folds_several_reviewers_and_the_pair_into_one_row_that_opens_beside_it() {
    // case: folds several reviewers and the pair into one row that opens beside it
    let row = item(&[BRO, SCOUT, CASPER], Some((&BRO, &SCOUT))).expect("a row");
    assert_eq!(row.label, "Ask for a review");
    assert_eq!(row.on_select, None);
    let submenu = row.submenu.expect("the reviewers open beside it");
    assert_eq!(submenu.iter().map(|one| one.label.as_str()).collect::<Vec<_>>(), ["Bro", "Scout", "Casper", "Bro (Claude Code) and Scout (OpenCode)"]);
    assert!(submenu[3].divider_above);
    assert!(!submenu[0].divider_above);
    let asked: Vec<&str> = [&submenu[1], &submenu[3]].iter().filter_map(|one| one.on_select.as_deref()).collect();
    assert_eq!(asked, ["Scout", "Bro+Scout"]);
    // One reviewer beside a pair is still the folded row: the lone row is for a reviewer with no pair.
    let beside = item(&[CASPER], Some((&BRO, &SCOUT))).expect("a row");
    assert_eq!(beside.label, "Ask for a review");
    assert_eq!(beside.submenu.map(|rows| rows.len()), Some(2));
}

#[test]
fn keeps_a_lone_reviewer_one_row_that_says_who_and_offers_nothing_with_nobody_to_ask() {
    // case: keeps a lone reviewer one row that says who, and offers nothing with nobody to ask
    let row = item(&[CASPER], None).expect("a row");
    assert_eq!(row.label, "Ask Casper for a review");
    assert_eq!(row.submenu, None);
    assert_eq!(row.on_select.as_deref(), Some("Casper"));
    assert_eq!(item(&[], None), None);
}
