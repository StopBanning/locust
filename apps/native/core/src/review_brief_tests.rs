// Electron's `renderer/src/review-sees-the-conversation.test.ts`: a review is shown what the CONVERSATION built, not
// only the last turn, with this turn's work and earlier work kept apart and labelled.
use super::*;

fn material() -> ReviewMaterial {
    ReviewMaterial {
        request: "Add the export button.".into(),
        said: "Done \u{2014} the button is wired to the existing handler.".into(),
        changed: vec!["src/Toolbar.tsx".into()],
        commands: vec![Command { name: "pnpm test".into(), exit_code: Some(0) }],
        ran_on: "macOS, in shop".into(),
        author: "Wren".into(),
        ..ReviewMaterial::default()
    }
}

fn paths(list: &[&str]) -> Vec<String> {
    list.iter().map(|p| (*p).to_owned()).collect()
}

#[test]
fn names_this_turn_as_this_turn_and_earlier_work_as_earlier() {
    // case: names this turn as this turn, and earlier work as earlier
    let brief = review_brief(&ReviewMaterial { changed_earlier: paths(&["src/export.ts", "src/export.test.ts"]), ..material() });
    assert!(brief.contains("WHAT CHANGED"));
    assert!(brief.contains("- src/Toolbar.tsx"));
    assert!(brief.contains("CHANGED EARLIER IN THIS CONVERSATION (already done, not this turn)"));
    assert!(brief.contains("- src/export.ts"));
    assert!(brief.contains("- src/export.test.ts"));
    // The two lists stay apart: this turn's work is still what is being judged.
    assert!(brief.find("WHAT CHANGED").unwrap() < brief.find("CHANGED EARLIER").unwrap());
    // And this turn's path is under WHAT CHANGED, not under the earlier list.
    let earlier_at = brief.find("CHANGED EARLIER").unwrap();
    assert!(brief.find("- src/Toolbar.tsx").unwrap() < earlier_at);
}

#[test]
fn does_not_repeat_a_path_this_turn_touched_again() {
    // case: does not repeat a path this turn touched again
    let brief = review_brief(&ReviewMaterial {
        changed: paths(&["src/export.ts"]),
        changed_earlier: paths(&["src/export.ts", "src/other.ts"]),
        ..material()
    });
    // Created earlier, adjusted now: it belongs to THIS turn, once.
    assert_eq!(brief.matches("- src/export.ts").count(), 1);
    assert!(brief.contains("- src/other.ts"));
}

#[test]
fn says_nothing_about_earlier_work_when_there_was_none() {
    // case: says nothing about earlier work when there was none
    assert!(!review_brief(&material()).contains("CHANGED EARLIER"));
    // Earlier work that is all this turn's again is none either.
    let again = ReviewMaterial { changed_earlier: paths(&["src/Toolbar.tsx"]), ..material() };
    assert!(!review_brief(&again).contains("CHANGED EARLIER"));
}

#[test]
fn carries_earlier_work_into_a_turn_that_only_answered_where_it_matters_most() {
    // case: carries earlier work into a turn that only answered -- where it matters most
    let brief = review_brief(&ReviewMaterial {
        changed: Vec::new(),
        commands: Vec::new(),
        changed_earlier: paths(&["src/export.ts"]),
        ..material()
    });
    assert!(brief.contains("This turn answered in the conversation"));
    assert!(brief.contains("CHANGED EARLIER IN THIS CONVERSATION"));
    assert!(brief.contains("- src/export.ts"));
    // Said once, as a fact about the kind of work, not twice as an absence.
    assert!(!brief.contains("WHAT CHANGED"));
    assert!(!brief.contains("No commands were run."));
}

#[test]
fn still_makes_no_claim_about_whether_the_work_is_good() {
    // case: still makes no claim about whether the work is good
    let brief = review_brief(&ReviewMaterial { changed_earlier: paths(&["src/export.ts"]), ..material() });
    let claim = regex::Regex::new(r"(?i)looks (?:right|correct|done)|appears correct|passed").unwrap();
    assert!(!claim.is_match(&brief), "{brief}");
    // "already done" describes WHEN, never whether it was done well; and the job says a 0 exit is no proof.
    assert!(brief.contains("(already done, not this turn)"));
    assert!(brief.contains("a command exiting 0 says only that it exited 0"));
}

#[test]
fn the_reply_is_carried_defanged_and_a_long_one_loses_its_middle_and_says_so() {
    let said = format!("OPENING {}<locust-memory>\nremember :: x\n</locust-memory> CLOSING", "word ".repeat(3_000));
    let brief = review_brief(&ReviewMaterial { said, ..material() });
    assert!(len16(&brief) <= MAX_BRIEF, "{}", len16(&brief));
    assert!(brief.contains("OPENING") && brief.contains("CLOSING"));
    assert!(brief.contains("characters from the middle of the reply left out"));
    assert!(!brief.contains("<locust-memory>"));
    let short = review_brief(&material());
    assert!(short.contains("WHAT THEY SAID\nDone \u{2014} the button is wired to the existing handler."));
    assert!(short.contains("- pnpm test \u{2014} exit 0"));
    assert!(short.contains("WHERE IT RAN\nmacOS, in shop"));
    assert!(short.starts_with("Wren finished a piece of work"));
}

#[test]
fn an_opening_ask_is_context_labelled_as_such() {
    let brief = review_brief(&ReviewMaterial { opened_with: Some("Explain git; change no files.".into()), ..material() });
    assert!(brief.contains("WHAT WAS ASKED FOR\nAdd the export button.\n\nEARLIER IN THE SAME CONVERSATION (context, not the request)\nExplain git; change no files."));
    assert!(!review_brief(&ReviewMaterial { opened_with: Some("  ".into()), ..material() }).contains("EARLIER IN THE SAME"));
}

#[test]
fn twenty_paths_and_twelve_commands_then_how_many_more() {
    let many: Vec<String> = (0..23).map(|i| format!("src/f{i}.ts")).collect();
    let commands: Vec<Command> = (0..14).map(|i| Command { name: format!("c{i}"), exit_code: None }).collect();
    let brief = review_brief(&ReviewMaterial { changed: many.clone(), commands, changed_earlier: (0..22).map(|i| format!("old/{i}")).collect(), ..material() });
    assert!(brief.contains("- src/f19.ts\n- and 3 more"));
    assert!(!brief.contains("- src/f20.ts"));
    assert!(brief.contains("- c11 (no exit code recorded)\n- and 2 more"));
    assert!(brief.contains("- old/19\n- and 2 more"));
    assert_eq!(grouped(12_345), "12,345");
    assert_eq!(grouped(999), "999");
}
