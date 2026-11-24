//! Electron's `at-picks-a-file-of-the-project.test.ts` (fileMentions.ts): what `@` is asking for, and the files offered.

use super::{at_query, file_matches, without_at_query};

const MAX: usize = 8;
fn files() -> Vec<String> {
    [
        "README.md",
        "src/cart.py",
        "src/cart_test.py",
        "src/checkout/cart_view.py",
        "docs/carting-notes.md",
        "apps/desktop/src/main/index.ts",
        "apps/desktop/src/renderer/src/App.tsx",
    ]
    .map(String::from)
    .to_vec()
}
fn matches(paths: &[String], query: &str) -> Vec<String> {
    file_matches(paths, query, MAX)
}

#[test]
fn is_the_at_word_at_the_end_of_the_message_a_bare_at_included() {
    assert_eq!(at_query("look at @car").as_deref(), Some("car"));
    assert_eq!(at_query("@").as_deref(), Some(""));
    assert_eq!(at_query("fix @src/ca").as_deref(), Some("src/ca"));
}

#[test]
fn is_nothing_for_an_at_inside_a_sentence_or_a_word_that_carries_one() {
    assert_eq!(at_query("email me @ noon please"), None);
    assert_eq!(at_query("write to sam@example.com"), None);
    assert_eq!(at_query("no at sign"), None);
}

#[test]
fn leaves_the_rest_of_the_message_when_a_file_is_picked() {
    assert_eq!(without_at_query("look at @car"), "look at ");
    assert_eq!(without_at_query("@README"), "");
}

#[test]
fn puts_a_name_that_starts_with_what_was_typed_before_one_that_only_contains_it() {
    assert_eq!(matches(&files(), "cart"), ["src/cart.py", "src/cart_test.py", "docs/carting-notes.md", "src/checkout/cart_view.py"]);
    assert_eq!(matches(&files(), "view"), ["src/checkout/cart_view.py"]);
}

#[test]
fn finds_by_path_and_by_letters_in_order() {
    assert_eq!(matches(&files(), "checkout/"), ["src/checkout/cart_view.py"]);
    assert_eq!(matches(&files(), "rndrapp"), ["apps/desktop/src/renderer/src/App.tsx"]);
}

#[test]
fn ignores_case_and_offers_the_shortest_paths_first_for_a_bare_at_eight_at_most() {
    assert_eq!(matches(&files(), "readme"), ["README.md"]);
    assert_eq!(matches(&files(), "")[0], "README.md");
    let thirty: Vec<String> = (0..30).map(|i| format!("f{i}.ts")).collect();
    assert_eq!(matches(&thirty, "").len(), 8);
}

#[test]
fn offers_nothing_rather_than_something_wrong() {
    assert!(matches(&files(), "zzzz").is_empty());
}

fn very_large() -> Vec<String> {
    let mut many: Vec<String> = (0..50_000).map(|i| format!("docs/part-{}/record-{i}.md", i % 97)).collect();
    many.push("packages/runtime-adapters/src/commands.ts".into());
    many
}

#[test]
fn finds_a_file_past_the_old_5000_cap_first() {
    assert_eq!(matches(&very_large(), "commands.ts")[0], "packages/runtime-adapters/src/commands.ts");
}

/// Was a 250 ms wall-clock limit, which a busy machine broke on unchanged code. What made it fast is the work it
/// skips, so that is what is checked: a bare `@` looks at each path once, lowers and folds none of them, and finds the
/// first eight with a bounded selection: about 2.4 comparisons a path today (119,704), under a ceiling of 4, where
/// sorting all 50,001 takes 740,160 (about 15 a path; both counted with the same comparator on this list).
#[test]
fn answers_a_bare_at_over_50000_paths_in_well_under_a_keystroke() {
    let many = very_large();
    let (shown, looked_at, lowered, compared) = super::work::count(|| matches(&many, ""));
    assert_eq!(shown.len(), 8);
    assert_eq!(looked_at, many.len(), "each path is looked at exactly once");
    assert_eq!(lowered, 0, "a bare @ lowers and folds no path");
    assert!(compared < 4 * many.len(), "{compared} comparisons for {} paths: the eight shown are selected, not every path sorted", many.len());
}

#[test]
fn finds_a_name_with_accents_from_one_typed_without_them_and_the_other_way_round() {
    let files: Vec<String> = ["docs/résumé.md", "docs/resume-old.md", "src/café.ts"].map(String::from).to_vec();
    assert_eq!(matches(&files, "resume"), ["docs/résumé.md", "docs/resume-old.md"]);
    assert_eq!(matches(&files, "café"), ["src/café.ts"]);
    assert_eq!(matches(&files, "cafe"), ["src/café.ts"]);
}
