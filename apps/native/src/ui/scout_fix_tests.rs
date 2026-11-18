//! Electron's `a-scout-fix-says-where-it-stands.test.tsx`, the cases Rust builds, with Electron's values and words:
//! the fix line (`scout_cards.rs`'s `render_fix_line`, drawn headless, with `scout::view::fix_said` and
//! `fix_files_line`) and whether a card keeps its drafted handoff beside a fix (`scout::view::card_shows_draft`, which
//! decides whether the card draws "Send to Wren").
use super::*;
use locust_core::scout::types::{ScoutCard, ScoutDraft, ScoutFact, ScoutFix, ScoutUrgency, card_of};
use locust_core::scout::view::card_shows_draft;
use rune::{Env, Runtime};

struct Harness {
    fix: ScoutFix,
}
fn view(h: &Harness, _: &mut Cx<Harness>) -> El {
    col().w_full().child(render_fix_line("card_1", &h.fix, &Look::new(true)))
}
/// The line's words, and its presses in the order drawn.
fn draw(fix: ScoutFix) -> (String, Vec<String>) {
    let mut rt = Runtime::new(Harness { fix }, view, (900.0, 300.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let words = all.iter().filter_map(|n| n.text.clone()).collect::<Vec<_>>().join("\n");
    let presses = all.iter().filter(|n| n.role == "button").filter_map(|n| n.label.clone()).collect();
    (words, presses)
}
const FIX_ID: &str = "rt_scoutfix0123456789ab";

#[test]
fn a_fix_says_each_step_in_words_with_only_the_presses_that_step_has() {
    let (said, _) = draw(ScoutFix::Preparing { fix_id: FIX_ID.into(), name: "Wren".into(), since: "2026-10-23T11:00:00Z".into() });
    assert!(said.contains("Wren is preparing a fix in a copy of the folder…"), "{said}");

    let (said, presses) = draw(ScoutFix::Ready { fix_id: FIX_ID.into(), name: "Wren".into(), files: vec!["src/answer.ts".into(), "test/answer.test.ts".into()],
        added: Some(3), removed: Some(1), mission_id: "mission_fix".into() });
    assert!(said.contains("Fix ready: 2 files changed (+3 −1), the check passes."), "{said}");
    assert!(said.contains("src/answer.ts · test/answer.test.ts"));
    assert_eq!(presses, ["Review", "Apply", "Skip"]);

    let (said, _) = draw(ScoutFix::Applied { files: vec!["src/answer.ts".into()] });
    assert!(said.contains("Applied: 1 file in your folder, beside your own changes. Opening a pull request is yours."), "{said}");

    let (said, presses) = draw(ScoutFix::Stale { fix_id: FIX_ID.into(), name: "Wren".into(), files: vec!["src/answer.ts".into()], mission_id: "mission_fix".into() });
    assert!(said.contains("Not applied: the fix was made on an older copy, and you changed that file since."), "{said}");
    assert!(presses.iter().any(|p| p == "Prepare again"));
    assert!(!presses.iter().any(|p| p == "Apply"));

    let (said, _) = draw(ScoutFix::Unfixed { name: "Wren".into(), why: "Wren changed nothing, and said: “The test needs a database I cannot reach.”".into(), mission_id: None });
    assert!(said.contains("Wren could not prepare a fix: Wren changed nothing, and said: “The test needs a database I cannot reach.”"), "{said}");

    let (_, presses) = draw(ScoutFix::Held { why: "Today’s fix is used, so Scout did not prepare one.".into() });
    assert!(presses.iter().any(|p| p == "Prepare again"));
}

fn card(fix: ScoutFix) -> ScoutCard {
    let fact = ScoutFact {
        draft: Some(ScoutDraft { teammate_id: "tm_wren".into(), name: "Wren".into(), message: "Fix the failing test.".into() }),
        ..ScoutFact::simple("check:m1:fails", ScoutUrgency::Now, "The check npm test fails: 1 failed.")
    };
    let at = chrono::DateTime::parse_from_rfc3339("2026-10-23T11:00:00Z").unwrap();
    ScoutCard { fix: Some(fix), ..card_of(fact, "card_1", at, false) }
}

#[test]
fn a_card_drops_the_handoff_drafted_for_the_same_check_while_the_fix_is_the_next_step_and_offers_it_again_when_it_could_not_be_made() {
    assert!(!card_shows_draft(&card(ScoutFix::Ready { fix_id: FIX_ID.into(), name: "Wren".into(), files: vec!["a.ts".into()], added: None, removed: None, mission_id: "m".into() })));
    assert!(!card_shows_draft(&card(ScoutFix::Preparing { fix_id: FIX_ID.into(), name: "Wren".into(), since: "2026-10-23T11:00:00Z".into() })));
    assert!(!card_shows_draft(&card(ScoutFix::Applied { files: vec!["a.ts".into()] })));
    assert!(card_shows_draft(&card(ScoutFix::Unfixed { name: "Wren".into(), why: "It ran past 30 minutes, so Locust stopped it.".into(), mission_id: None })));
}
