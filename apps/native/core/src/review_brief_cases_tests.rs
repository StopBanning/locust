//! review-brief.test.ts (20 cases) and a-review-brief-fits-and-asks-for-a-verdict.test.ts (4 cases): the brief a
//! reviewer is handed, case for case. Lengths are counted in UTF-16 units, as JavaScript's `length` counts them.
use super::*;
use crate::hand_off::len16;

fn material() -> ReviewMaterial {
    ReviewMaterial {
        request: "Make the path handler work on Windows.".into(),
        said: "I normalised the separators in resolve() and added two cases for drive letters.".into(),
        changed: vec!["src/paths.ts".into(), "src/paths.test.ts".into()],
        commands: vec![
            Command { name: "pnpm test".into(), exit_code: Some(0) },
            Command { name: "tsc --noEmit".into(), exit_code: Some(0) },
        ],
        ran_on: "Windows \u{b7} locust-astra".into(),
        author: "Jimothy".into(),
        ..Default::default()
    }
}

fn answered() -> ReviewMaterial {
    ReviewMaterial {
        request: "Research the company Keel.".into(),
        said: "Keel is a private logistics company founded in 2019. Revenue is not disclosed\u{2026}".into(),
        changed: vec![],
        commands: vec![],
        ..material()
    }
}

fn later() -> ReviewMaterial {
    ReviewMaterial {
        request: "Create alpha.txt and beta.txt, then print them joined.".into(),
        opened_with: Some("I'm new to git. Explain the working tree. Please only answer; don't change any files.".into()),
        ..material()
    }
}

#[test]
fn carries_the_request_in_the_words_the_person_used() {
    // case: carries the request in the words the person used
    assert!(review_brief(&material()).contains("Make the path handler work on Windows."));
}

#[test]
fn carries_what_changed_what_ran_and_where() {
    // case: carries what changed, what ran with its exit code, and where
    let brief = review_brief(&material());
    assert!(brief.contains("src/paths.ts"));
    assert!(brief.contains("pnpm test \u{2014} exit 0"));
    assert!(brief.contains("Windows \u{b7} locust-astra"));
}

#[test]
fn names_who_did_the_work_and_that_the_reviewer_did_not() {
    // case: names who did the work, and that the reviewer did not
    let brief = review_brief(&material());
    assert!(brief.contains("Jimothy"));
    assert!(brief.contains("You did not do this work"));
}

#[test]
fn makes_no_claim_that_the_work_is_done_or_correct() {
    // case: MAKES NO CLAIM THAT THE WORK IS DONE OR CORRECT
    let brief = review_brief(&material()).to_lowercase();
    for claim in ["passed", "all exit 0", "verified", "looks correct", "successfully", "working correctly"] {
        assert!(!brief.contains(claim), "{claim}");
    }
    assert!(brief.contains("not proof that the work is correct"));
}

#[test]
fn tells_it_to_report_rather_than_fix() {
    // case: tells it to report rather than fix
    let brief = review_brief(&material());
    assert!(brief.contains("Do not redo the work"));
    assert!(brief.contains("edit no file: report; do not fix."));
}

#[test]
fn says_a_short_answer_is_right_when_the_work_is_fine() {
    // case: says a short answer is right when the work is fine
    assert!(review_brief(&material()).contains("say so plainly and stop"));
}

#[test]
fn says_plainly_when_a_run_changed_nothing() {
    // case: says plainly when a run changed nothing, rather than leaving a blank
    assert!(review_brief(&ReviewMaterial { changed: vec![], ..material() }).contains("Nothing in the workspace."));
}

#[test]
fn says_plainly_when_nothing_was_run() {
    // case: says plainly when nothing was run
    assert!(review_brief(&ReviewMaterial { commands: vec![], ..material() }).contains("No commands were run."));
}

#[test]
fn marks_a_command_whose_exit_code_was_never_recorded() {
    // case: marks a command whose exit code was never recorded
    let brief = review_brief(&ReviewMaterial { commands: vec![Command { name: "pnpm test".into(), exit_code: None }], ..material() });
    assert!(brief.contains("(no exit code recorded)"));
    assert!(!brief.contains("exit 0"));
}

#[test]
fn bounds_a_long_list_rather_than_truncating_it_silently() {
    // case: bounds a long list rather than truncating it silently
    let many = (0..25).map(|i| format!("src/file{i}.ts")).collect();
    let brief = review_brief(&ReviewMaterial { changed: many, ..material() });
    assert!(brief.contains("and 5 more"));
    assert!(brief.contains("src/file19.ts") && !brief.contains("src/file20.ts"));
}

#[test]
fn an_answer_carries_what_the_teammate_said() {
    // case: carries what the teammate said
    let brief = review_brief(&answered());
    assert!(brief.contains("WHAT THEY SAID"));
    assert!(brief.contains("Keel is a private logistics company"));
}

#[test]
fn an_answer_is_never_presented_as_the_two_bare_absences() {
    // case: never presents it as the two bare absences
    let brief = review_brief(&answered());
    assert!(!brief.contains("Nothing in the workspace."));
    assert!(!brief.contains("No commands were run."));
    assert!(brief.contains("answered in the conversation"));
}

#[test]
fn an_answer_is_said_to_be_the_work() {
    // case: tells the reviewer that answering IS the work
    assert!(review_brief(&answered()).contains("A turn that only answered is a complete piece of work."));
}

#[test]
fn an_answer_is_asked_about_as_work_not_a_change() {
    // case: asks about the work rather than about the change
    let brief = review_brief(&answered());
    assert!(brief.contains("Say whether the work does what was asked for"));
    assert!(!brief.contains("Say whether the change does"));
}

#[test]
fn still_leads_a_real_code_change_with_what_changed() {
    // case: still leads a real code change with what changed
    let brief = review_brief(&material());
    assert!(brief.contains("WHAT CHANGED"));
    assert!(brief.contains("src/paths.ts"));
    assert!(brief.find("WHAT THEY SAID").unwrap() < brief.find("WHAT CHANGED").unwrap());
}

#[test]
fn says_out_loud_when_the_reply_was_cut() {
    // case: says out loud when the reply was cut, rather than ending mid-sentence
    let said = format!("It opened like this. {} And it ended with the answer: 42.", "y".repeat(20_000));
    let brief = review_brief(&ReviewMaterial { said, ..material() });
    assert!(brief.contains("It opened like this."));
    assert!(brief.contains("And it ended with the answer: 42."));
    let mark = crate::invariants::regex(r"\[\.\.\. [\d,]+ characters from the middle of the reply left out; the whole reply is in their thread \.\.\.\]");
    assert!(mark.is_match(&brief));
    assert!(len16(&brief) <= 8_000);
}

#[test]
fn bounds_a_very_long_request_with_a_mark() {
    // case: bounds a very long request, with a mark
    let brief = review_brief(&ReviewMaterial { request: "x".repeat(5_000), ..material() });
    assert!(brief.contains('\u{2026}'));
    assert!(len16(&brief) < 3_000);
}

#[test]
fn a_moved_on_conversation_asks_about_this_turn() {
    // case: asks about THIS turn, not the one the conversation opened with
    let brief = review_brief(&later());
    let asked = brief.find("WHAT WAS ASKED FOR").unwrap();
    let context = brief.find("EARLIER IN THE SAME CONVERSATION").unwrap();
    assert!(context > asked);
    assert!(brief[asked..context].contains("Create alpha.txt"));
    assert!(!brief[asked..context].contains("only answer"));
}

#[test]
fn a_moved_on_conversation_keeps_the_opening_ask_as_context() {
    // case: keeps the opening ask, labelled as context rather than as the request
    let brief = review_brief(&later());
    assert!(brief.contains("EARLIER IN THE SAME CONVERSATION (context, not the request)"));
    assert!(brief.contains("don't change any files"));
}

#[test]
fn says_nothing_extra_when_the_conversation_has_not_moved_on() {
    // case: says nothing extra when the conversation has not moved on
    assert!(!review_brief(&material()).contains("EARLIER IN THE SAME CONVERSATION"));
}

fn long() -> ReviewMaterial {
    ReviewMaterial {
        request: "r".repeat(5_000),
        opened_with: Some("o".repeat(5_000)),
        said: format!("First, what I did. {} Finally: the fix is in src/net.ts and the tests pass.", "s".repeat(30_000)),
        changed: (0..40).map(|i| format!("src/file-{i}.ts")).collect(),
        commands: (0..30).map(|i| Command { name: format!("pnpm test {i}"), exit_code: Some(0) }).collect(),
        changed_earlier: (0..40).map(|i| format!("docs/page-{i}.md")).collect(),
        ran_on: "Claude Code \u{b7} Haiku \u{b7} Accept edits \u{b7} in the folder".into(),
        author: "Wren".into(),
    }
}

#[test]
fn fits_in_a_mission_prompt_however_long_the_turn_was() {
    // case: fits in a mission prompt however long the turn was
    let brief = review_brief(&long());
    assert!(len16(&brief) <= STEP_BUDGET);
    assert!(brief.contains("First, what I did."));
    assert!(brief.contains("Finally: the fix is in src/net.ts and the tests pass."));
    assert!(brief.contains("characters from the middle of the reply left out"));
}

#[test]
fn leaves_a_reply_that_fits_exactly_as_it_was() {
    // case: leaves a reply that fits exactly as it was
    let brief = review_brief(&ReviewMaterial {
        request: "Fix the null check.".into(),
        opened_with: None,
        said: "Fixed it in src/net.ts.".into(),
        changed: vec!["src/net.ts".into()],
        commands: vec![],
        changed_earlier: vec![],
        ..long()
    });
    assert!(brief.contains("WHAT THEY SAID\nFixed it in src/net.ts.\n"));
    assert!(!brief.contains("left out"));
}

#[test]
fn cuts_from_the_middle_and_counts_what_it_cut() {
    // case: cuts from the middle, and counts what it cut
    let cut = trimmed_from_the_middle(&format!("HEAD {} TAIL", "x".repeat(5_000)), 1_000);
    assert!(len16(&cut) <= 1_000);
    assert!(cut.starts_with("HEAD"));
    assert!(cut.ends_with("TAIL"));
    let counted = crate::invariants::regex(r"\[\.\.\. ([\d,]+) characters").captures(&cut).unwrap()[1].replace(',', "");
    assert!(counted.parse::<usize>().unwrap() > 4_000);
}

#[test]
fn asks_for_a_verdict_first_and_no_edits_and_does_not_take_the_author_at_their_word() {
    // case: asks for a verdict first, and for no edits, and does not take the author at their word
    let brief = review_brief(&long());
    assert!(brief.contains("Start your answer with a one-line verdict -- Ready, Needs changes, or Start over -- then the reasons."));
    assert!(brief.contains("edit no file"));
    assert!(brief.contains("What they said about their own work is their account, not evidence"));
}
