// Carries apps/desktop/src/shared/a-point-can-count-twice.test.ts and points-are-drafted.test.ts (Electron 4ce2f37a).
use crate::compare::{judge_mission_ids, parsed_compare_file};
use crate::compare_score::{
    DRAFT_MAX_POINTS, criteria_lines, criteria_weights, drafted_points, panel_vote, parse_scorecard, points_draft_request, score_of, weighted,
};

const CRITERIA: &str = "Names a price for five seats ×2\n- Says which CRM to buy\nFits in one sentence (x3)\nKeeps it short x9";
const SAID: &str = "SCORE A 1: met. yes\nSCORE A 2: not met. no\nSCORE A 3: met. yes\nSCORE A 4: met. yes\nSCORE B 1: not met. no\nSCORE B 2: met. yes\nSCORE B 3: met. yes\nSCORE B 4: met. yes";

#[test]
fn a_points_weight_is_read_off_the_end_of_its_line_and_the_judges_get_the_point_without_it() {
    assert_eq!(criteria_lines(Some(CRITERIA)), ["Names a price for five seats", "Says which CRM to buy", "Fits in one sentence", "Keeps it short x9"]);
    assert_eq!(criteria_weights(Some(CRITERIA)), [2, 1, 3, 1]);
}

#[test]
fn the_points_met_are_scored_by_their_weight() {
    let card = weighted(parse_scorecard(SAID, &["a", "b"], &criteria_lines(Some(CRITERIA))), &criteria_weights(Some(CRITERIA))).unwrap();
    assert_eq!(card.weights.as_deref(), Some(&[2, 1, 3, 1][..]));
    let weights = card.weights.as_deref();
    assert_eq!(score_of(card.verdicts.get("a").map(Vec::as_slice), weights), (6, 7));
    assert_eq!(score_of(card.verdicts.get("b").map(Vec::as_slice), weights), (5, 7));
    assert_eq!(score_of(card.verdicts.get("a").map(Vec::as_slice), None), (3, 4));
}

#[test]
fn a_scorecard_of_points_that_all_count_once_stays_as_it_was_and_weights_survive_a_vote() {
    let plain = parse_scorecard(SAID, &["a", "b"], &criteria_lines(Some("One\nTwo\nThree\nFour")));
    assert_eq!(weighted(plain, &criteria_weights(Some("One\nTwo\nThree\nFour"))).and_then(|c| c.weights), None);
    let card = weighted(parse_scorecard(SAID, &["a", "b"], &criteria_lines(Some(CRITERIA))), &criteria_weights(Some(CRITERIA))).unwrap();
    assert_eq!(panel_vote(&[Some(&card), Some(&card), Some(&card)]).and_then(|c| c.weights), Some(vec![2, 1, 3, 1]));
}

#[test]
fn a_size_or_a_name_that_ends_in_a_number_is_never_read_as_a_weight() {
    let in_words = ["The result is a 4x4", "Works on Linux2", "Draws a 3×3 grid", "Handles empty input x3", "Covers the edge cases(x2)"].join("\n");
    assert_eq!(criteria_lines(Some(&in_words)), ["The result is a 4x4", "Works on Linux2", "Draws a 3×3 grid", "Handles empty input", "Covers the edge cases"]);
    assert_eq!(criteria_weights(Some(&in_words)), [1, 1, 1, 3, 2]);
}

#[test]
fn the_points_draft_asks_for_points_only_in_the_point_form_about_the_request() {
    let asked = points_draft_request("Which CRM should a five-person team buy?");
    assert!(asked.contains("Which CRM should a five-person team buy?"));
    assert!(asked.contains("Do not answer it."));
    assert!(asked.lines().any(|l| l.starts_with("POINT 1: <")), "{asked}");
}

#[test]
fn drafted_points_are_read_in_order_each_once_and_at_most_five() {
    let said = [
        "Here are the points:",
        "- **POINT 1:** Names one CRM to buy",
        "POINT 2 - Gives a price for five seats",
        "POINT 2: said twice, so not kept",
        "POINT 4: out of order, so not kept",
        "POINT 3: Fits a five-person team",
        "POINT 4: Says why it beats the others",
        "POINT 5: One sentence",
        "POINT 6: one too many",
    ]
    .join("\n");
    assert_eq!(drafted_points(&said), ["Names one CRM to buy", "Gives a price for five seats", "Fits a five-person team", "Says why it beats the others", "One sentence"]);
    assert_eq!(drafted_points(&said).len(), DRAFT_MAX_POINTS);
    assert!(drafted_points("I would buy HubSpot.").is_empty());
}

// Electron's case gives the comparison no slots; Rust's reader refuses a comparison of fewer than two, so it has two.
#[test]
fn a_points_drafting_run_is_the_comparisons_never_a_conversation() {
    let raw = serde_json::json!({ "schemaVersion": 1, "compares": [{ "compareId": "cmp_1", "prompt": "Which CRM?", "createdAt": "2026-10-12T18:00:00.000Z",
        "slots": [{ "slot": "a", "route": { "runtime": "codex", "model": "gpt-6" }, "missionIds": [] },
                  { "slot": "b", "route": { "runtime": "claude", "model": "sonnet" }, "missionIds": [] }],
        "pointDrafts": ["m_draft"] }] });
    let compares = parsed_compare_file(&raw.to_string()).unwrap();
    assert!(judge_mission_ids(&compares).contains("m_draft"));
}
