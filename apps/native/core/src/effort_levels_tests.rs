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
