//! Electron's `shared/attachments.test.ts`, one Rust test for each of its ten cases, in its order, with its values and
//! its sentences. Tests named `beyond_electron_` hold what its source does and its test is silent on.
use super::*;

fn paths(of: &[&str]) -> Vec<String> {
    of.iter().map(|p| (*p).to_owned()).collect()
}

fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

// describe('naming attached files in a message')

#[test]
fn draws_nothing_at_all_when_nothing_is_attached() {
    // So a caller cannot prepend an empty instruction to every message.
    assert_eq!(attachment_preamble(&[]), None);
    assert_eq!(with_attachments("Fix the parser.", &[]), "Fix the parser.");
}

#[test]
fn names_one_file_and_asks_for_it_to_be_read() {
    let said = attachment_preamble(&paths(&["src/parser.ts"])).unwrap();
    assert!(said.contains("src/parser.ts"));
    assert!(said.to_lowercase().contains("read this file"));
}

#[test]
fn counts_them_when_there_are_several() {
    let said = attachment_preamble(&paths(&["a.ts", "b.ts", "c.ts"])).unwrap();
    assert!(said.to_lowercase().contains("read these 3 files"));
    for path in ["a.ts", "b.ts", "c.ts"] {
        assert!(said.contains(path));
    }
}

#[test]
fn puts_the_instruction_above_what_the_person_typed() {
    // The runtime should know what it is working from before it reads the request, not after.
    let sent = with_attachments("Fix the parser.", &paths(&["src/parser.ts"]));
    assert!(sent.find("src/parser.ts").unwrap() < sent.find("Fix the parser.").unwrap());
    assert!(sent.ends_with("Fix the parser."));
}

#[test]
fn never_inlines_a_file_however_tempting() {
    // THE test. Pasting contents would eat the prompt budget that inbound peer messages already lose to. A path costs
    // a line.
    let sent = with_attachments("Summarise it.", &paths(&["notes.md"]));
    assert!(utf16(&sent) < 200);
}

#[test]
fn says_file_not_attached_because_on_most_runtimes_nothing_is() {
    // Five of six get a reference, not an attachment. A word that overstates what happened is the thing this app keeps
    // refusing to do.
    assert_eq!(attachment_label(1), "1 file");
    assert_eq!(attachment_label(3), "3 files");
    assert!(!attachment_label(1).to_lowercase().contains("attach"));
}

// describe('reading a sent prompt back for display')

#[test]
fn gives_back_exactly_what_with_attachments_put_in() {
    // The round trip is the whole contract: if these two ever disagree, the bubble shows the host's instruction again.
    for set in [paths(&["notes.md"]), paths(&["a.ts", "b.ts"]), paths(&["src/deep/file name.md", "x.json", "y.csv"])] {
        let split = split_attachments(&with_attachments("Fix the parser.", &set));
        assert_eq!(split.0, "Fix the parser.");
        assert_eq!(split.1, set);
    }
}

#[test]
fn leaves_a_prompt_with_no_attachments_completely_alone() {
    assert_eq!(split_attachments("Fix the parser."), ("Fix the parser.".to_owned(), Vec::new()));
    assert_eq!(split_attachments(&with_attachments("Fix the parser.", &[])), ("Fix the parser.".to_owned(), Vec::new()));
}

#[test]
fn keeps_a_multi_line_message_intact_under_the_preamble() {
    let typed = "Do this:\n\n- first\n- second\n";
    let split = split_attachments(&with_attachments(typed, &paths(&["plan.md"])));
    assert_eq!(split.0, typed);
    assert_eq!(split.1, paths(&["plan.md"]));
}

#[test]
fn refuses_to_strip_something_it_did_not_write() {
    // A prompt this cannot account for is one to leave whole. Stripping a near-miss would delete words the person
    // typed.
    let nearly = "Read this file in the workspace before you answer:\nnotes.md\n\nWhat is in it?";
    assert_eq!(split_attachments(nearly).0, nearly);
    assert_eq!(split_attachments(nearly).1, Vec::<String>::new());
    let no_words = "Read this file in the workspace before you answer:\n- notes.md\n";
    assert_eq!(split_attachments(no_words).0, no_words);
}

// What Electron's source does and its test is silent on (values taken from running `shared/attachments.ts`).

#[test]
fn beyond_electron_a_name_with_a_line_break_stays_one_quoted_item() {
    // QA-2026-09-29 round 2, R20: a name holding a line break wrote a line of its own into the brief.
    let said = attachment_preamble(&paths(&["a\nb", "c\u{7f}d", "e\tf", "g\"h\\i"])).unwrap();
    assert_eq!(
        said,
        "Read these 4 files in the workspace before you answer:\n- \"a\\nb\"\n- \"c\u{7f}d\"\n- \"e\\tf\"\n- g\"h\\i"
    );
}

#[test]
fn beyond_electron_a_note_goes_between_the_preamble_and_the_words_only_when_it_has_some() {
    assert_eq!(
        with_attachments_note("go", &paths(&["a"]), Some("NOTE")),
        "Read this file in the workspace before you answer:\n- a\n\nNOTE\n\ngo"
    );
    assert_eq!(with_attachments_note("go", &paths(&["a"]), Some("")), "Read this file in the workspace before you answer:\n- a\n\ngo");
    assert_eq!(with_attachments_note("go", &[], Some("NOTE")), "go");
}

#[test]
fn beyond_electron_the_split_reads_the_regular_expression_as_javascript_does() {
    // An empty message after a preamble: the match gives the list's last newline back, so the words are that newline.
    assert_eq!(split_attachments(&with_attachments("", &paths(&["a"]))), ("\n".to_owned(), paths(&["a"])));
    // `\d` is ASCII only; zero is a count; a carriage return stays in the name; extra blank lines belong to the words.
    assert_eq!(
        split_attachments("Read these 0 files in the workspace before you answer:\n- a\n\nhi"),
        ("hi".to_owned(), paths(&["a"]))
    );
    let arabic = "Read these \u{663} files in the workspace before you answer:\n- a\n\nhi";
    assert_eq!(split_attachments(arabic), (arabic.to_owned(), Vec::new()));
    assert_eq!(
        split_attachments("Read this file in the workspace before you answer:\n- a\r\n- b\n\nhi"),
        ("hi".to_owned(), paths(&["a\r", "b"]))
    );
    assert_eq!(
        split_attachments("Read this file in the workspace before you answer:\n- a\n\n\nhi"),
        ("\nhi".to_owned(), paths(&["a"]))
    );
}

#[test]
fn beyond_electron_the_split_takes_what_the_expression_takes_and_no_more() {
    // Run through Electron's source: a list line needs `- ` and one more character, a list may end without its newline
    // when the words follow at once, and the opening must start the prompt.
    assert_eq!(
        split_attachments("Read this file in the workspace before you answer:\n- a\n- b\n\nhi"),
        ("hi".to_owned(), paths(&["a", "b"]))
    );
    assert_eq!(
        split_attachments("Read this file in the workspace before you answer:\n-  \n\nhi"),
        ("hi".to_owned(), paths(&[" "]))
    );
    assert_eq!(
        split_attachments("Read this file in the workspace before you answer:\n- a\nhi"),
        ("hi".to_owned(), paths(&["a"]))
    );
    let empty_count = "Read these  files in the workspace before you answer:\n- a\n\nhi";
    assert_eq!(split_attachments(empty_count), (empty_count.to_owned(), Vec::new()));
    let not_first = "x Read this file in the workspace before you answer:\n- a\n\nhi";
    assert_eq!(split_attachments(not_first), (not_first.to_owned(), Vec::new()));
}

#[test]
fn beyond_electron_a_name_is_quoted_from_nul_to_unit_separator_and_delete_and_not_beyond() {
    // Electron's `/[\u0000-\u001f\u007f]/`: the ends of the range, and the characters just outside it.
    let said = attachment_preamble(&paths(&["a\u{0}b", "a\u{1f}b", "a\u{7f}b", "a b", "a\u{80}b", "a\u{7e}b"])).unwrap();
    assert_eq!(
        said,
        "Read these 6 files in the workspace before you answer:\n- \"a\\u0000b\"\n- \"a\\u001fb\"\n- \"a\u{7f}b\"\n- a b\n- a\u{80}b\n- a~b"
    );
}

#[test]
fn beyond_electron_a_count_of_ten_or_more_files_is_read_back() {
    // `\d+`, not `\d`: the count in the opening has as many digits as the number of files.
    let many: Vec<String> = (1..=12).map(|n| format!("f{n}.md")).collect();
    let split = split_attachments(&with_attachments("Fix the parser.", &many));
    assert_eq!(split, ("Fix the parser.".to_owned(), many));
}

#[test]
fn beyond_electron_the_caps_and_the_folder_are_electrons() {
    assert_eq!(MAX_ATTACHMENTS, 8);
    assert_eq!(MAX_PASTED_BYTES, 24 * 1024 * 1024);
    assert_eq!(ATTACHMENT_DIR, ".locust/attachments");
}
