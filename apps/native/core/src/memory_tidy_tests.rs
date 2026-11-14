//! Electron's shared/a-tidy-block-is-read-exactly.test.ts (8) and shared/a-fenced-tidy-block-still-counts.test.ts (6).
use super::*;
use crate::protocol_tags::defang_protocol_blocks;

fn block(lines: &[&str]) -> String {
    let mut all = vec!["Found three near-copies.", "<locust-tidy>"];
    all.extend_from_slice(lines);
    all.push("</locust-tidy>");
    all.join("\n")
}

fn merge(ids: &[&str], text: &str) -> TidySuggestion {
    TidySuggestion::Merge { ids: ids.iter().map(|s| s.to_string()).collect(), text: text.into() }
}
fn retire(id: &str, reason: &str) -> TidySuggestion {
    TidySuggestion::Retire { id: id.into(), reason: reason.into() }
}

// --- a-tidy-block-is-read-exactly.test.ts ---

#[test]
fn reading_the_block_reads_a_merge_a_retirement_and_a_rewrite_ids_exactly() {
    assert_eq!(
        parse_tidy_blocks(&block(&["merge mem_a1 mem_b2 :: Deploys go out on Thursdays.", "retire mem_c3 :: superseded by the Thursday note", "rewrite mem_d4 :: The API is on port 3001."])),
        vec![
            merge(&["mem_a1", "mem_b2"], "Deploys go out on Thursdays."),
            retire("mem_c3", "superseded by the Thursday note"),
            TidySuggestion::Rewrite { id: "mem_d4".into(), text: "The API is on port 3001.".into() },
        ]
    );
}

#[test]
fn reading_the_block_drops_what_does_not_read_cleanly() {
    let too_many = (0..=MAX_MERGED).map(|i| format!("mem_x{i}")).collect::<Vec<_>>().join(" ");
    let too_many_line = format!("merge {too_many} :: too many");
    assert_eq!(
        parse_tidy_blocks(&block(&[
            "merge mem_a1 :: one is not a merge",
            &too_many_line,
            "merge mem_a1 mem_a1 :: the same one twice",
            "rewrite mem_a1 mem_b2 :: which one?",
            "retire :: nothing named",
            "shred mem_a1 :: not a verb this knows",
        ])),
        vec![]
    );
}

#[test]
fn reading_the_block_takes_at_most_a_handful_from_one_reply() {
    let lines: Vec<String> = (0..MAX_TIDY_SUGGESTIONS + 5).map(|i| format!("retire mem_r{i} :: stale")).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    assert_eq!(parse_tidy_blocks(&block(&refs)).len(), MAX_TIDY_SUGGESTIONS);
}

#[test]
fn reading_the_block_does_not_act_on_the_example_in_its_own_brief_fenced_or_not() {
    assert_eq!(parse_tidy_blocks(&tidy_prompt()), vec![]);
    let fenced = ["```", "<locust-tidy>", TIDY_EXAMPLE_LINES[0], TIDY_EXAMPLE_LINES[1], TIDY_EXAMPLE_LINES[2], "</locust-tidy>", "```"].join("\n");
    assert_eq!(parse_tidy_blocks(&format!("Like this:\n{fenced}")), vec![]);
}

#[test]
fn the_block_is_taken_out_of_the_reply_a_person_reads() {
    assert_eq!(strip_memory_blocks(&block(&["retire mem_a1 :: stale"])), "Found three near-copies.");
}

#[test]
fn the_block_is_defanged_when_quoted_to_another_teammate() {
    assert!(!defang_protocol_blocks("<locust-tidy>retire mem_a1 :: stale</locust-tidy>").contains("<locust-tidy>"));
}

const KEPT: [(&str, &str); 1] = [("mem_moved", "The API moved to port 3001.")];

#[test]
fn a_reason_drops_an_id_cited_in_brackets_the_sentence_already_says_it() {
    assert_eq!(readable_reason("moved to port 3001 on September 20 (mem_moved), making this outdated", &KEPT), "moved to port 3001 on September 20, making this outdated");
}

#[test]
fn a_reason_says_a_bare_id_as_the_memory_it_names_or_as_another_memory() {
    assert_eq!(readable_reason("superseded by mem_moved", &KEPT), "superseded by \"The API moved to port 3001.\"");
    assert_eq!(readable_reason("superseded by mem_gone", &KEPT), "superseded by another memory");
}

// --- a-fenced-tidy-block-still-counts.test.ts ---

fn wren() -> String {
    [
        "Looking at the memory file, I found several duplicate pairs and superseded entries. Here are my suggestions:",
        "",
        "```",
        "<locust-tidy>",
        "merge mem_deploy_a mem_deploy_b :: We deploy every Thursday afternoon.",
        "merge mem_pnpm_a mem_pnpm_b :: The package manager for this repo is pnpm.",
        "retire mem_port_old :: Superseded by mem_port_new, API moved to port 3001 on September 20.",
        "retire mem_todo :: Deadline (Friday August 29) has passed; no longer actionable.",
        "</locust-tidy>",
        "```",
    ]
    .join("\n")
}

#[test]
fn a_tidy_block_in_a_code_fence_is_read_like_any_other() {
    assert_eq!(
        parse_tidy_blocks(&wren()),
        vec![
            merge(&["mem_deploy_a", "mem_deploy_b"], "We deploy every Thursday afternoon."),
            merge(&["mem_pnpm_a", "mem_pnpm_b"], "The package manager for this repo is pnpm."),
            retire("mem_port_old", "Superseded by mem_port_new, API moved to port 3001 on September 20."),
            retire("mem_todo", "Deadline (Friday August 29) has passed; no longer actionable."),
        ]
    );
}

#[test]
fn a_tidy_block_in_a_code_fence_is_asked_once_when_it_is_shown_and_then_written_again() {
    let twice = format!("{}\n\nOr, plainly:\n<locust-tidy>\nretire mem_todo :: Deadline has passed.\n</locust-tidy>", wren());
    assert_eq!(parse_tidy_blocks(&twice).iter().filter(|s| matches!(s, TidySuggestion::Retire { id, .. } if id == "mem_todo")).count(), 1);
}

#[test]
fn a_tidy_block_in_a_code_fence_leaves_no_empty_box_behind_in_the_reply_a_person_reads() {
    assert_eq!(strip_memory_blocks(&wren()), "Looking at the memory file, I found several duplicate pairs and superseded entries. Here are my suggestions:");
}

#[test]
fn what_the_fence_still_protects_against_is_the_briefs_own_example_which_proposes_nothing_wherever_it_appears() {
    assert_eq!(parse_tidy_blocks(&tidy_prompt()), vec![]);
    assert_eq!(parse_tidy_blocks(&["<locust-tidy>", TIDY_EXAMPLE_LINES[0], TIDY_EXAMPLE_LINES[1], TIDY_EXAMPLE_LINES[2], "</locust-tidy>"].join("\n")), vec![]);
}

#[test]
fn the_brief_no_longer_shows_its_example_to_the_model_in_a_fence() {
    let prompt = tidy_prompt();
    assert!(!prompt.contains("```"));
    assert!(prompt.contains("Write the block in your reply itself -- not in a code block, a command or a file."));
    assert!(prompt.contains(&["<locust-tidy>", TIDY_EXAMPLE_LINES[0], TIDY_EXAMPLE_LINES[1], TIDY_EXAMPLE_LINES[2], "</locust-tidy>"].join("\n")));
}

#[test]
fn a_memory_block_in_a_code_fence_was_not_acted_on_so_it_stays_in_the_reply_for_the_person_to_read() {
    let shown = "An example:\n```\n<locust-memory>\nremember :: Deploys go out on Thursdays.\n</locust-memory>\n```";
    assert_eq!(strip_memory_blocks(shown), shown);
    assert_eq!(strip_memory_blocks("Noted.\n<locust-memory>\nremember :: Deploys go out on Thursdays.\n</locust-memory>"), "Noted.");
}

// Native's own: the reply the window draws, and the tidy pass's prompt recognised.

#[test]
fn the_reply_the_window_draws_never_shows_a_memory_or_tidy_blocks_words() {
    let reply = "Noted.\n<locust-memory>\nremember :: Deploys go out on Thursdays.\n</locust-memory>\n\nAnd:\n<locust-tidy>\nretire mem_a1 :: stale\n</locust-tidy>";
    let shown = crate::protocol_tags::shown_text(reply);
    assert!(!shown.contains("remember ::") && !shown.contains("retire mem_a1"), "{shown}");
    assert!(shown.starts_with("Noted."));
}

#[test]
fn a_tidy_pass_is_known_by_its_briefs_first_line() {
    assert!(is_tidy_prompt(Some(&tidy_prompt())));
    assert!(is_tidy_prompt(Some(&format!("{}\nold fenced example", tidy_prompt().lines().next().unwrap()))));
    assert!(!is_tidy_prompt(Some("Tidy up the README")));
    assert!(!is_tidy_prompt(None));
}

#[test]
fn a_line_that_could_not_be_read_is_counted_and_a_blank_example_or_repeat_is_not() {
    let (found, unread) = read_tidy_blocks(&block(&["", TIDY_EXAMPLE_LINES[1], "retire mem_a :: stale", "retire mem_a :: stale", "shred mem_b :: no", "merge mem_c :: one"]));
    assert_eq!(found, vec![retire("mem_a", "stale")]);
    assert_eq!(unread, 2);
}

#[test]
fn a_bullet_or_a_number_in_front_of_a_line_is_how_a_model_writes_a_list_not_part_of_it() {
    assert_eq!(
        parse_tidy_blocks(&block(&["- retire mem_a :: stale", "2) merge mem_b mem_c :: One sentence.", "• rewrite mem_d :: Fixed."])),
        vec![retire("mem_a", "stale"), merge(&["mem_b", "mem_c"], "One sentence."), TidySuggestion::Rewrite { id: "mem_d".into(), text: "Fixed.".into() }]
    );
}

#[test]
fn a_reason_closes_the_space_an_id_left_before_its_punctuation() {
    assert_eq!(readable_reason("stale (mem_moved) . See mem_gone , too", &KEPT), "stale. See another memory, too");
}
