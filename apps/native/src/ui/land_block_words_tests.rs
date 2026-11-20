//! a-landing-says-why-it-cannot.test.ts (2 cases): every reason a branch cannot land is a sentence naming the teammate
//! or the files, and says what would change it. Rust's `busy` is a flag beside the block, as Electron's own block.
use super::*;
use locust_core::worktrees::LandBlock;

fn said(block: Option<&LandBlock>, busy: bool) -> String {
    block_sentence(block, busy, "Wren", Some("main")).expect("a sentence")
}

// case: is a sentence for every reason, naming the teammate or the files
#[test]
fn every_reason_is_a_sentence_naming_the_teammate_or_the_files() {
    let every: Vec<(Option<LandBlock>, bool, Option<&str>)> = vec![
        (Some(LandBlock::Nothing), false, None),
        (None, true, None),
        (Some(LandBlock::Detached), false, None),
        (Some(LandBlock::Merging), false, None),
        (Some(LandBlock::OldGit(Some("2.30.1".into()))), false, None),
        (Some(LandBlock::Unsaved(vec!["scratch.txt".into()])), false, Some("scratch.txt")),
        (Some(LandBlock::Markers(vec!["cart.py".into()])), false, Some("cart.py")),
        (Some(LandBlock::YourChanges(vec!["cart.py".into(), "README.md".into(), "a.py".into(), "b.py".into()])), false, Some("cart.py")),
        (Some(LandBlock::Conflicts(vec!["cart.py".into()])), false, Some("cart.py")),
        (Some(LandBlock::MarkersUnchecked), false, None),
    ];
    for (block, busy, file) in &every {
        let words = said(block.as_ref(), *busy);
        assert!(words.len() > 20 && words.ends_with('.'), "{words}");
        if let Some(file) = file {
            assert!(words.contains(file), "{words}");
        }
    }
}

// case: says what would change it, where the person can act
#[test]
fn says_what_would_change_it() {
    assert_eq!(said(Some(&LandBlock::YourChanges(vec!["cart.py".into()])), false), "You have unsaved changes in cart.py, which this landing would overwrite. Commit them or set them aside, then read it again.");
    assert_eq!(said(None, true), "Wren is working. Land it when the turn ends.");
    assert_eq!(said(Some(&LandBlock::Conflicts(vec!["cart.py".into()])), false), "cart.py was changed on main too, in the same places.");
    assert!(said(Some(&LandBlock::OldGit(Some("2.30.1".into()))), false).contains("this machine has 2.30.1"));
    assert!(said(Some(&LandBlock::YourChanges(["a", "b", "c", "d"].map(String::from).to_vec())), false).contains("a, b and 2 more"));
}
