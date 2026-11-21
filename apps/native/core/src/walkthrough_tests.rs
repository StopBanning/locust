//! Electron's `main/a-change-is-walked-through.test.ts`, with its exact values.

use super::*;

fn diff() -> String {
    [
        "diff --git a/src/total.js b/src/total.js",
        "--- a/src/total.js",
        "+++ b/src/total.js",
        "@@ -1 +1 @@",
        "-export const total = (lines) => lines.reduce((sum, line) => sum - line.amount, 0)",
        "+export const total = (lines) => lines.reduce((sum, line) => sum + line.amount, 0)",
        "diff --git a/test/total.test.js b/test/total.test.js",
        "new file mode 100644",
        "--- /dev/null",
        "+++ b/test/total.test.js",
        "@@ -0,0 +1 @@",
        "+console.assert(total([{ amount: 2 }, { amount: 3 }]) === 5)",
        "",
    ]
    .join("\n")
}

fn paths() -> Vec<String> {
    vec!["src/total.js".into(), "test/total.test.js".into()]
}

// case: what the teammate is given > is the files, the change, what was asked and what it said, with the shape to answer in
#[test]
fn is_the_files_the_change_what_was_asked_and_what_it_said_with_the_shape_to_answer_in() {
    let d = diff();
    assert_eq!(changed_paths(&d), paths());
    let prompt = walkthrough_prompt(&d, false, &paths(), &["Fix the total".into()], Some("It subtracted; it adds now."));
    assert!(prompt.contains("do not read files or run commands"));
    assert!(prompt.contains("What the person asked for:\n- Fix the total"));
    assert!(prompt.contains("What was said when the work was done:\nIt subtracted; it adds now."));
    assert!(prompt.contains("The files changed:\n- src/total.js\n- test/total.test.js"));
    assert!(prompt.contains(&format!("The change:\n{d}")));
    assert!(prompt.ends_with("END"));
    // A change past the limit is cut, and says so.
    let big = walkthrough_prompt(&format!("{d}{}", "+x\n".repeat(MAX_WALK_DIFF_CHARS)), false, &paths(), &[], None);
    assert!(big.contains("(cut short; the list of files above is whole)"));
    assert!(big.len() < MAX_WALK_DIFF_CHARS + 3_000);
}

#[test]
fn asks_only_the_last_five_and_says_nothing_of_an_empty_ask_or_account() {
    let asks: Vec<String> = (1..=7).map(|n| format!("ask  {n}\n")).chain([" ".to_owned()]).collect();
    let prompt = walkthrough_prompt("", false, &[], &asks, Some("  "));
    assert!(prompt.contains("What the person asked for:\n- ask 3\n- ask 4\n- ask 5\n- ask 6\n- ask 7\n\n"));
    assert!(!prompt.contains("ask 2"));
    assert!(!prompt.contains("What was said"));
    assert!(walkthrough_prompt("x", true, &[], &[], None).contains("The change (cut short; the list of files above is whole):\nx"));
    assert!(!walkthrough_prompt("x", false, &[], &[], None).contains("What the person asked for"));
}

#[test]
fn names_each_changed_file_once_in_order() {
    let d = "diff --git a/a b/b b/x\r\ndiff --git a/one b/one\ndiff --git a/one b/one\nnot diff --git a/z b/z\n";
    assert_eq!(changed_paths(d), vec!["b b/x".to_owned(), "one".to_owned()]);
    // `a\/(.+?) b\/`: the a side has at least one character, so a " b/" right after "a/" is not the split.
    assert_eq!(changed_paths("diff --git a/ b/x b/y\n"), vec!["y".to_owned()]);
}

// case: the walkthrough in the answer > is the summary and a note on each file the change has, in the order given
#[test]
fn is_the_summary_and_a_note_on_each_file_the_change_has_in_the_order_given() {
    let answer = [
        "Sure, here it is.",
        "SUMMARY: The total now adds each line",
        "instead of subtracting it, and a test holds it there.",
        "FILE: test/total.test.js",
        "WHY: A new test: two lines of 2 and 3 come to 5.",
        "FILE: `src/total.js` (modified)",
        "WHY: The reducer added a minus sign; it adds now.",
        "Check the empty invoice too: it comes to 0.",
        "CHECK: An invoice with no lines.",
        "END",
        "Anything else?",
    ]
    .join("\n");
    assert_eq!(
        read_walkthrough(&answer, &paths()),
        Some(Walkthrough {
            summary: "The total now adds each line instead of subtracting it, and a test holds it there.".into(),
            files: vec![
                WalkthroughFile { path: "test/total.test.js".into(), why: "A new test: two lines of 2 and 3 come to 5.".into(), check: None },
                // A line of the note that begins "Check the..." is still the note; the label needs its colon.
                WalkthroughFile {
                    path: "src/total.js".into(),
                    why: "The reducer added a minus sign; it adds now. Check the empty invoice too: it comes to 0.".into(),
                    check: Some("An invoice with no lines.".into()),
                },
            ],
        })
    );
}

// case: the walkthrough in the answer > finds a file by its end or its name, and leaves out one the change does not have
#[test]
fn finds_a_file_by_its_end_or_its_name_and_leaves_out_one_the_change_does_not_have() {
    let answer = [
        "**SUMMARY:** Fixes the total.",
        "**FILE:** total.js",
        "**WHY:** Adds.",
        "FILE: src/billing/invoice.js",
        "WHY: Invented.",
        "FILE: ./test/total.test.js",
        "WHY: Tests it.",
        "FILE: test/total.test.js",
        "WHY: Twice.",
    ]
    .join("\n");
    let walk = read_walkthrough(&answer, &paths()).unwrap();
    assert_eq!(walk.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), vec!["src/total.js", "test/total.test.js"]);
    assert_eq!(walk.files[1].why, "Tests it.");
    assert_eq!(walk.summary, "Fixes the total.");
}

// case: the walkthrough in the answer > is nothing without a summary
#[test]
fn is_nothing_without_a_summary() {
    assert_eq!(read_walkthrough("FILE: src/total.js\nWHY: Adds.\nEND", &paths()), None);
    assert_eq!(read_walkthrough("The total adds now.", &paths()), None);
}

#[test]
fn reads_a_path_as_git_or_a_quote_writes_it_and_none_two_files_share() {
    let two = vec!["a/x/total.js".to_owned(), "b/y/total.js".to_owned(), "lib/one.rs".to_owned()];
    let walk = read_walkthrough(
        "summary: s\nfile: \"b/lib/one.rs\"\nwhy: w\nFILE: total.js\nWHY: which?\nFILE: y/total.js\nWHY: this one\n * END * \nFILE: lib/one.rs\nWHY: after the end",
        &two,
    )
    .unwrap();
    assert_eq!(
        walk.files,
        vec![
            WalkthroughFile { path: "lib/one.rs".into(), why: "w".into(), check: None },
            WalkthroughFile { path: "b/y/total.js".into(), why: "this one".into(), check: None },
        ]
    );
    // A WHY or CHECK before any FILE is nobody's; a FILE with no note is left out; an "End users" line is not END.
    let walk = read_walkthrough("SUMMARY: s\nWHY: stray\nmore\nFILE: lib/one.rs\nCHECK: c\nFILES: no\nEnd users see it", &two).unwrap();
    assert!(walk.files.is_empty());
    // The stray WHY is not a label there, so it is read as more of the summary, as Electron's reader does.
    assert_eq!(walk.summary, "s WHY: stray more");
    // A path as git writes it, a/ or b/ in front, where nothing but the prefix finds it.
    let same_name = vec!["src/total.js".to_owned(), "lib/total.js".to_owned()];
    let walk = read_walkthrough("SUMMARY: s\nFILE: a/src/total.js\nWHY: w", &same_name).unwrap();
    assert_eq!(walk.files[0].path, "src/total.js");
    let walk = read_walkthrough("SUMMARY: s\nFILE: lib/one.rs\nWHY: w\nEnd users see it\nCHECK: c", &two).unwrap();
    assert_eq!(walk.files[0].why, "w End users see it");
    assert_eq!(walk.files[0].check.as_deref(), Some("c"));
}

#[test]
fn clips_a_long_summary_and_note_with_an_ellipsis() {
    let long = "word ".repeat(300);
    let walk = read_walkthrough(&format!("SUMMARY: {long}\nFILE: lib/one.rs\nWHY: {long}\nCHECK: {long}"), &["lib/one.rs".into()]).unwrap();
    assert_eq!(walk.summary.chars().count(), 700);
    assert!(walk.summary.ends_with("word…"));
    assert!(walk.files[0].why.chars().count() <= 400 && walk.files[0].why.ends_with('…'));
    assert!(walk.files[0].check.as_ref().unwrap().ends_with('…'));
    assert_eq!(clip("abc", 3), "abc");
    assert_eq!(clip("abcd", 3), "ab…");
    assert_eq!(clip("a   bcd", 3), "a…");
}

#[test]
fn shows_the_files_in_the_order_given_and_the_rest_in_gits_order_after() {
    let walk = Walkthrough {
        summary: "s".into(),
        files: vec![
            WalkthroughFile { path: "c".into(), why: "first".into(), check: None },
            WalkthroughFile { path: "a".into(), why: "second".into(), check: Some("k".into()) },
        ],
    };
    let paths = ["a", "b", "c", "d"];
    assert_eq!(reading_order(&paths, Some(&walk)), vec![2, 0, 1, 3]);
    assert_eq!(reading_order(&paths, None), vec![0, 1, 2, 3]);
    assert_eq!(note_on(&walk, "a").map(|(n, f)| (n, f.why.as_str())), Some((2, "second")));
    assert_eq!(note_on(&walk, "c").map(|(n, _)| n), Some(1));
    assert!(note_on(&walk, "b").is_none());
}
