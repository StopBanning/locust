//! Electron's `effortLevels.test.ts` and `the-effort-thumb-follows-the-pointer.test.ts`, one test per case, with their
//! values; and the word and geometry cases of `the-effort-control-is-claude-codes.test.tsx` that need no window.
use super::*;

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.005
}

// --- effortLevels.test.ts: describing a reasoning-effort level

#[test]
fn describes_the_levels_runtimes_actually_list() {
    for level in ["low", "medium", "high", "high-fast", "max", "fast"] {
        assert!(effort_description(level).is_some(), "{level}");
    }
}

#[test]
fn says_nothing_for_a_level_it_does_not_know() {
    assert_eq!(effort_description("turbo-ultra"), None);
    assert_eq!(effort_description(""), None);
}

#[test]
fn talks_about_time_and_cost_not_about_how_good_the_answer_will_be() {
    let all = ["low", "medium", "high", "max"].map(|l| effort_description(l).unwrap_or("")).join(" ").to_lowercase();
    assert!(["fast", "slow", "cheap", "costlier"].iter().any(|w| all.contains(w)), "{all}");
    assert!(!["better", "smarter", "best answer"].iter().any(|w| all.contains(w)), "{all}");
}

#[test]
fn is_not_case_sensitive_because_runtimes_are_not_consistent() {
    assert_eq!(effort_description("HIGH"), effort_description("high"));
    assert!(effort_description("HIGH").is_some());
}

// --- effortLevels.test.ts: the note under the menu

#[test]
fn says_how_cursor_actually_takes_an_effort() {
    assert!(effort_footer("cursor").unwrap_or("").to_lowercase().contains("in the model name"));
}

#[test]
fn draws_no_note_for_a_runtime_it_has_nothing_true_to_say_about() {
    for runtime in ["claude", "codex", "opencode", "copilot"] {
        assert_eq!(effort_footer(runtime), None, "{runtime}");
    }
}

// --- the-effort-control-is-claude-codes.test.tsx: a level

#[test]
fn a_level_is_named_in_words_as_claude_code_names_it() {
    assert_eq!(effort_name("max"), "Max");
    assert_eq!(effort_name("xhigh"), "Extra high");
    assert_eq!(effort_name("medium"), "Medium");
}

#[test]
fn a_level_this_build_does_not_know_keeps_the_runtimes_own_word_capitalised_never_renamed() {
    assert_eq!(effort_name("turbo"), "Turbo");
    assert_eq!(effort_name(""), "");
}

// --- the-effort-control-is-claude-codes.test.tsx: the control's geometry and its liquid

#[test]
fn puts_the_stops_end_to_end_along_the_track() {
    assert!(stop_position(0, 5) > 0.0);
    assert!(stop_position(4, 5) < EFFORT_TRACK_WIDTH);
    assert!(approx(stop_position(0, 5) + stop_position(4, 5), EFFORT_TRACK_WIDTH));
    assert_eq!(stop_position(0, 1), EFFORT_TRACK_WIDTH / 2.0);
}

#[test]
fn leaves_no_tail() {
    assert_eq!(MOVE.trail, 0.0);
    assert!(MOVE.stretch > 0.0);
}

// --- the-effort-thumb-follows-the-pointer.test.ts: a held thumb

#[test]
fn a_held_thumb_is_where_the_pointer_is_along_the_track() {
    assert_eq!(held_at(100.0 + 90.0, 100.0), 90.0);
}

#[test]
fn a_held_thumb_never_leaves_the_stops_before_the_first_or_past_the_last_it_waits_at_the_end() {
    assert_eq!(held_at(100.0 - 40.0, 100.0), stop_position(0, 4));
    assert_eq!(held_at(100.0 + EFFORT_TRACK_WIDTH + 40.0, 100.0), stop_position(3, 4));
}

// --- the-effort-thumb-follows-the-pointer.test.ts: letting go

#[test]
fn letting_go_settles_on_the_nearest_stop_whichever_side_of_it_the_thumb_is() {
    let between = (stop_position(1, 4) + stop_position(2, 4)) / 2.0;
    assert_eq!(nearest_stop(stop_position(1, 4), 4), 1);
    assert_eq!(nearest_stop(between - 1.0, 4), 1);
    assert_eq!(nearest_stop(between + 1.0, 4), 2);
    assert_eq!(nearest_stop(stop_position(3, 4), 4), 3);
}

#[test]
fn letting_go_never_names_a_stop_that_is_not_there() {
    assert_eq!(nearest_stop(-50.0, 4), 0);
    assert_eq!(nearest_stop(EFFORT_TRACK_WIDTH + 50.0, 4), 3);
    assert_eq!(nearest_stop(80.0, 1), 0);
}

// --- effortScale.ts, which the slider stands on (its own Electron test is not in this batch)

#[test]
fn cursors_eight_levels_are_four_stops_and_a_fast_switch_sorted_least_to_most() {
    let listed: Vec<String> = ["high-fast", "low", "low-fast", "medium", "medium-fast", "high", "xhigh", "xhigh-fast"].map(String::from).to_vec();
    let scale = effort_scale(&listed);
    assert_eq!(scale.bases, ["low", "medium", "high", "xhigh"]);
    assert!(scale.has_fast);
    assert_eq!(join_effort("medium", true, &listed).as_deref(), Some("medium-fast"));
    let plain: Vec<String> = ["low", "high"].map(String::from).to_vec();
    assert_eq!(join_effort("high", true, &plain).as_deref(), Some("high"), "no fast variant: the plain one, never an invented id");
    assert_eq!(join_effort("max", false, &plain), None);
    assert_eq!(levels_line(&listed).as_deref(), Some("Low to Extra high · Fast"));
    assert_eq!(levels_line(&plain).as_deref(), Some("Low, High"));
    assert_eq!(levels_line(&[]), None);
}

/// effortScale: an unfamiliar word goes after every known one, so it cannot land between `low` and `medium`.
#[test]
fn puts_a_word_it_does_not_know_after_every_known_level() {
    let scale = effort_scale(&["turbo".into(), "high".into(), "low".into()]);
    assert_eq!(scale.bases, vec!["low", "high", "turbo"]);
}

/// levelsLine: three levels are named one by one; four or more read "first to last".
#[test]
fn names_three_levels_and_spans_four() {
    let levels = |l: &[&str]| levels_line(&l.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
    assert_eq!(levels(&["low", "medium", "high"]), "Low, Medium, High");
    assert_eq!(levels(&["low", "medium", "high", "max"]), "Low to Max");
}

// --- routeChipReading.test.ts: the model and its effort read as one control

#[test]
fn joins_them_the_way_the_reference_does() {
    assert_eq!(route_chip_reading("Haiku 5.5", Some("Medium")), "Haiku 5.5 · Medium");
    assert_eq!(route_chip_reading("Opus 5.5", Some("High · Fast")), "Opus 5.5 · High · Fast");
    assert_eq!(effort_phrase("medium"), "Medium");
    assert_eq!(effort_phrase("high-fast"), "High · Fast");
}

#[test]
fn leaves_the_name_alone_when_there_is_no_effort_to_state() {
    assert_eq!(route_chip_reading("Haiku 5.5", None), "Haiku 5.5");
    assert_eq!(route_chip_reading("Haiku 5.5", Some("  ")), "Haiku 5.5");
}

// --- effortScale.test.ts: reading a runtime list of efforts as a scale

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

const CURSOR_SORTED: &[&str] = &["low", "low-fast", "medium", "medium-fast", "high", "high-fast", "xhigh", "xhigh-fast"];
const CLAUDE_LEVELS: &[&str] = &["low", "medium", "high", "xhigh", "max"];
const CURSOR_UNSORTED: &[&str] = &["high-fast", "low", "low-fast", "medium", "medium-fast", "high", "xhigh", "xhigh-fast"];

// case: turns Cursor eight levels into four and a switch
#[test]
fn turns_cursor_eight_levels_into_four_and_a_switch() {
    let scale = effort_scale(&strings(CURSOR_SORTED));
    assert_eq!(scale, EffortScale { bases: strings(&["low", "medium", "high", "xhigh"]), has_fast: true });
}

// case: draws no switch for a runtime with no fast variants
#[test]
fn draws_no_switch_for_a_runtime_with_no_fast_variants() {
    let scale = effort_scale(&strings(CLAUDE_LEVELS));
    assert_eq!(scale, EffortScale { bases: strings(CLAUDE_LEVELS), has_fast: false });
}

// case: keeps the runtime own order, because right means more
#[test]
fn keeps_the_runtime_own_order_because_right_means_more() {
    assert_eq!(effort_scale(&strings(&["tiny", "enormous"])).bases, strings(&["tiny", "enormous"]));
}

// case: is not fooled by a level that merely ends in the word fast
#[test]
fn is_not_fooled_by_a_level_that_merely_ends_in_the_word_fast() {
    assert_eq!(split_effort("fast"), ("fast", false));
    assert_eq!(split_effort("-fast"), ("-fast", false));
    assert_eq!(split_effort("high-fast"), ("high", true));
}

// --- effortScale.test.ts: turning the slider and the switch back into a real level

// case: gives the fast variant when one exists
#[test]
fn gives_the_fast_variant_when_one_exists() {
    let cursor = strings(CURSOR_SORTED);
    assert_eq!(join_effort("high", true, &cursor).as_deref(), Some("high-fast"));
    assert_eq!(join_effort("high", false, &cursor).as_deref(), Some("high"));
}

// case: NEVER invents a level the runtime did not list
#[test]
fn never_invents_a_level_the_runtime_did_not_list() {
    let claude = strings(CLAUDE_LEVELS);
    assert_eq!(join_effort("medium", true, &claude).as_deref(), Some("medium"));
    assert_eq!(join_effort("nonsense", true, &claude), None);
    assert_eq!(join_effort("nonsense", false, &claude), None);
}

// case: round-trips every level a runtime actually offers
#[test]
fn round_trips_every_level_a_runtime_actually_offers() {
    for list in [strings(CURSOR_SORTED), strings(CLAUDE_LEVELS)] {
        for level in &list {
            let (base, fast) = split_effort(level);
            assert_eq!(join_effort(base, fast, &list).as_deref(), Some(level.as_str()), "{level}");
        }
    }
}

// --- effortScale.test.ts: a scale where right is more

// case: sorts a list the runtime did not sort
#[test]
fn sorts_a_list_the_runtime_did_not_sort() {
    assert_eq!(effort_scale(&strings(CURSOR_UNSORTED)).bases, strings(&["low", "medium", "high", "xhigh"]));
}

// case: puts high above medium, which is the whole bug
#[test]
fn puts_high_above_medium_which_is_the_whole_bug() {
    let bases = effort_scale(&strings(CURSOR_UNSORTED)).bases;
    let high = bases.iter().position(|b| b == "high").unwrap();
    let medium = bases.iter().position(|b| b == "medium").unwrap();
    assert!(high > medium);
    assert_ne!(high, 0);
}

// case: leaves an already-sorted list alone
#[test]
fn leaves_an_already_sorted_list_alone() {
    assert_eq!(effort_scale(&strings(&["low", "medium", "high"])).bases, strings(&["low", "medium", "high"]));
    assert_eq!(
        effort_scale(&strings(&["none", "low", "medium", "high", "xhigh", "max"])).bases,
        strings(&["none", "low", "medium", "high", "xhigh", "max"])
    );
}

// case: keeps a word it has never seen after the ones it knows, in the runtime order
#[test]
fn keeps_a_word_it_has_never_seen_after_the_ones_it_knows_in_the_runtime_order() {
    let bases = effort_scale(&strings(&["ludicrous", "high", "low", "brisk"])).bases;
    assert_eq!(bases, strings(&["low", "high", "ludicrous", "brisk"]));
}

// case: still finds the fast variants wherever they were listed
#[test]
fn still_finds_the_fast_variants_wherever_they_were_listed() {
    assert!(effort_scale(&strings(CURSOR_UNSORTED)).has_fast);
    assert!(!effort_scale(&strings(&["low", "medium"])).has_fast);
}

// case: a level the scale holds can always be found on it
#[test]
fn a_level_the_scale_holds_can_always_be_found_on_it() {
    let bases = effort_scale(&strings(CURSOR_UNSORTED)).bases;
    for base in ["low", "medium", "high", "xhigh"] {
        assert!(bases.iter().position(|b| b == base).is_some(), "{base}");
    }
}
