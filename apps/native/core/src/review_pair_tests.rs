//! review-pair.test.ts (3 cases): two reviewers on different coding agents (A3.1), case for case.
use super::*;

struct T {
    name: &'static str,
    runtime: &'static str,
}

impl ReviewCandidate for T {
    fn runtime(&self) -> &str {
        self.runtime
    }
}

fn t(name: &'static str, runtime: &'static str) -> T {
    T { name, runtime }
}

fn names(pair: Option<(&T, &T)>) -> Option<[&'static str; 2]> {
    pair.map(|(a, b)| [a.name, b.name])
}

#[test]
fn picks_two_agents_that_differ_from_each_other_and_from_the_author() {
    // case: picks two agents that differ from each other and from the author
    let team = [t("Ash", "codex"), t("Yurt", "codex"), t("Booty", "claude"), t("Vale", "opencode")];
    assert_eq!(names(review_pair_of(&team, Some("codex"))), Some(["Booty", "Vale"]));
}

#[test]
fn uses_the_authors_agent_for_one_of_them_when_the_team_has_nothing_else() {
    // case: uses the author's agent for one of them when the team has nothing else
    let team = [t("Ash", "codex"), t("Booty", "claude")];
    assert_eq!(names(review_pair_of(&team, Some("codex"))), Some(["Ash", "Booty"]));
}

#[test]
fn offers_no_pair_when_every_reviewer_is_on_the_same_agent() {
    // case: offers no pair when every reviewer is on the same agent
    assert_eq!(names(review_pair_of(&[t("Ash", "claude"), t("Booty", "claude")], Some("codex"))), None);
    assert_eq!(names(review_pair_of(&[t("Ash", "claude")], Some("codex"))), None);
}
