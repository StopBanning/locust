//! Electron's `test/decision.test.ts` (24 cases), with Electron's blocks, values and words: a runtime asking which way
//! to go, the question taken out of the bubble, the answer sent back, and a permission request refused as a decision.
//! Lengths are counted in UTF-16 units, as Electron counts them.
use super::*;

fn block(body: &str) -> String {
    format!("Here is what I found.\n\n<locust-ask>\n{body}\n</locust-ask>")
}

fn well_formed() -> String {
    block("The v2 handler has two callers outside billing. Keep them on v2 behind\nthe flag, or migrate them in this mission?\n- Keep them on v2 :: Smaller change, flag stays until you flip it\n- Migrate all callers now :: Touches 4 more files, adds ~10 min")
}

fn opt(label: &str, note: Option<&str>) -> DecisionOption {
    DecisionOption { label: label.into(), note: note.map(str::to_owned) }
}

fn u16len(s: &str) -> usize {
    s.encode_utf16().count()
}

// ---- a runtime asking which way to go ----

#[test]
fn reads_the_question_and_both_options() {
    let asked = parse_decision(&well_formed()).unwrap();
    assert_eq!(asked.question, "The v2 handler has two callers outside billing. Keep them on v2 behind the flag, or migrate them in this mission?");
    assert_eq!(asked.options, vec![
        opt("Keep them on v2", Some("Smaller change, flag stays until you flip it")),
        opt("Migrate all callers now", Some("Touches 4 more files, adds ~10 min")),
    ]);
}

#[test]
fn drops_the_template_brackets_a_model_kept_and_leaves_arrows_and_comparisons_alone() {
    let asked = parse_decision(&block("How should I finalize your plan without your numbers?\n- <option name>I'll supply my numbers :: reply with starting savings and costs, then I finalize the plan>\n- <Use an illustrative example> :: <I build it on sample figures>\n- Keep it small :: diffs stay < 50 lines, then a -> b")).unwrap();
    assert_eq!(asked.options, vec![
        opt("I'll supply my numbers", Some("reply with starting savings and costs, then I finalize the plan")),
        opt("Use an illustrative example", Some("I build it on sample figures")),
        opt("Keep it small", Some("diffs stay < 50 lines, then a -> b")),
    ]);
}

#[test]
fn accepts_an_option_that_states_no_cost() {
    let asked = parse_decision(&block("Which one?\n- Do it now\n- Wait for review")).unwrap();
    assert_eq!(asked.options.iter().map(|o| o.note.clone()).collect::<Vec<_>>(), vec![None, None]);
}

#[test]
fn takes_an_asterisk_bullet_as_readily_as_a_dash() {
    assert_eq!(parse_decision(&block("Which one?\n* First\n* Second")).unwrap().options.len(), 2);
}

#[test]
fn refuses_a_single_option_which_is_an_announcement_rather_than_a_decision() {
    assert!(parse_decision(&block("Shall I proceed?\n- Yes go ahead")).is_none());
}

#[test]
fn refuses_more_options_than_a_person_will_read() {
    assert!(parse_decision(&block("Which?\n- One\n- Two\n- Three\n- Four\n- Five")).is_none());
}

#[test]
fn refuses_two_options_that_say_the_same_thing() {
    assert!(parse_decision(&block("Which?\n- Keep it\n- keep it")).is_none());
}

#[test]
fn refuses_a_block_with_options_but_no_question() {
    assert!(parse_decision(&block("- One\n- Two")).is_none());
}

#[test]
fn refuses_a_block_with_a_question_but_no_options() {
    assert!(parse_decision(&block("What should I do here?")).is_none());
}

#[test]
fn ignores_prose_that_arrives_after_the_options() {
    assert_eq!(parse_decision(&block("Which one?\n- One\n- Two\nI can start whenever you say.")).unwrap().question, "Which one?");
}

#[test]
fn takes_only_the_first_well_formed_question() {
    let two = format!("{}\n{}", well_formed(), block("And after that?\n- Ship it\n- Hold it"));
    assert!(parse_decision(&two).unwrap().question.contains("v2 handler"));
}

#[test]
fn skips_a_malformed_block_to_find_a_good_one_after_it() {
    let text = format!("{}\n{}", block("Broken, only one\n- Only"), well_formed());
    assert!(parse_decision(&text).unwrap().question.contains("v2 handler"));
}

#[test]
fn finds_nothing_in_an_ordinary_reply() {
    assert!(parse_decision("I changed two files and the tests pass.").is_none());
}

#[test]
fn strips_a_control_byte_rather_than_refusing_the_whole_question() {
    assert_eq!(parse_decision(&block("What now?\u{7}\n- One\n- Two")).unwrap().question, "What now?");
}

#[test]
fn cuts_an_overlong_question_rather_than_showing_a_memo() {
    let long = parse_decision(&block(&format!("{}?\n- One\n- Two", "word ".repeat(400)))).unwrap();
    assert_eq!(u16len(&long.question), MAX_QUESTION_LENGTH);
    assert!(long.question.ends_with('\u{2026}'));
}

#[test]
fn cuts_an_overlong_option_label() {
    let long = parse_decision(&block(&format!("Which?\n- {}\n- Two", "x".repeat(300)))).unwrap();
    assert_eq!(u16len(&long.options[0].label), MAX_OPTION_LABEL_LENGTH);
}

// ---- the question in the reply bubble ----

#[test]
fn is_taken_out_because_the_card_asks_it_answerably() {
    let stripped = strip_decision_blocks(&well_formed());
    assert_eq!(stripped, "Here is what I found.");
    assert!(!stripped.contains("locust-ask"));
}

#[test]
fn leaves_an_ordinary_reply_exactly_as_it_was() {
    assert_eq!(strip_decision_blocks("No question here."), "No question here.");
}

#[test]
fn takes_out_a_malformed_block_too_so_no_raw_tag_ever_reaches_the_reader() {
    let broken = block("Only one\n- Only");
    assert!(parse_decision(&broken).is_none());
    assert!(!strip_decision_blocks(&broken).contains("locust-ask"));
}

// ---- what the next turn is sent when a person picks ----

#[test]
fn reads_as_the_answer_the_person_gave() {
    assert_eq!(decision_reply(&opt("Keep them on v2", Some("Smaller change"))), "Keep them on v2 (Smaller change). Continue with that.");
}

#[test]
fn says_the_label_alone_when_the_option_stated_no_cost() {
    assert_eq!(decision_reply(&opt("Wait for review", None)), "Wait for review. Continue with that.");
}

// ---- a permission request wearing the decision card ----

#[test]
fn refuses_a_yes_no_question_because_that_is_an_approval() {
    assert!(parse_decision(&block("Shall I proceed with the migration?\n- Yes\n- No")).is_none());
}

// case: refuses %s
#[test]
fn refuses_each_assent_and_dissent_pair_however_it_is_written() {
    for (label, options) in [
        ("proceed / cancel", "- Proceed\n- Cancel"),
        ("go ahead / stop", "- Go ahead\n- Stop"),
        ("confirm / abort", "- Confirm\n- Abort"),
        ("punctuated", "- Yes.\n- No!"),
        ("mixed case", "- YES\n- no"),
        ("one bare among substantive", "- Migrate all callers now\n- No"),
    ] {
        assert!(parse_decision(&block(&format!("Should I?\n{options}"))).is_none(), "refuses {label}");
    }
}

#[test]
fn keeps_a_real_fork_whose_option_merely_begins_with_an_assent_word() {
    let asked = parse_decision(&block("How far should the migration go?\n- Yes, and migrate the callers too :: touches 4 files\n- Keep them on v2 :: smaller change")).unwrap();
    assert_eq!(asked.options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), vec!["Yes, and migrate the callers too", "Keep them on v2"]);
}

#[test]
fn keeps_options_that_name_what_to_do_even_when_one_sounds_decisive() {
    assert_eq!(parse_decision(&block("Which?\n- Stop the retry after one minute\n- Continue retrying three times")).unwrap().options.len(), 2);
}
