//! Electron's `the-picker-opens-on-what-works.test.ts`, `the-chip-keeps-free.test.ts`, the function cases of
//! `older-claude-versions-are-folded.test.tsx`, `a-model-is-one-line.test.tsx`'s words,
//! `a-busy-free-model-offers-the-next.test.tsx`'s `nextFreeModel` and `a-free-model-that-says-nothing-is-offered-another`'s
//! `quietFreeRun`, one test per case with their values.
use super::*;

fn row(group: &str, key: &str, tag: &str) -> RouteRow {
    RouteRow { key: key.into(), group: group.into(), model: None, tag: Some(tag.into()) }
}

fn groups(rows: &[RouteRow]) -> Vec<&str> {
    order_route_rows(rows, &[]).into_iter().map(|i| rows[i].group.as_str()).collect()
}

// --- the-picker-opens-on-what-works.test.ts

#[test]
fn lifts_the_one_runtime_that_can_run_above_the_three_that_cannot() {
    let rows = [
        row("Codex CLI · your account", "codex:gpt-5.6", "NOT INSTALLED"),
        row("Claude Code", "claude:sonnet", "NOT INSTALLED"),
        row("Cursor Agent", "cursor:composer", "NOT INSTALLED"),
        row("Gemini CLI", "gemini:flash", "PLANNED"),
        row("OpenCode", "opencode:ling-free", "READY"),
    ];
    assert_eq!(groups(&rows), ["OpenCode", "Claude Code", "Codex CLI · your account", "Cursor Agent", "Gemini CLI"]);
}

#[test]
fn sinks_what_does_not_exist_below_what_merely_is_not_installed() {
    let rows = [row("Gemini CLI", "g", "PLANNED"), row("Claude Code", "c", "NOT INSTALLED")];
    assert_eq!(groups(&rows), ["Claude Code", "Gemini CLI"]);
}

#[test]
fn keeps_a_signed_out_runtime_above_one_that_is_not_on_the_machine_at_all() {
    let rows = [row("Claude Code", "c", "NOT INSTALLED"), row("Codex CLI", "x", "SIGN IN")];
    assert_eq!(groups(&rows), ["Codex CLI", "Claude Code"]);
}

#[test]
fn takes_a_group_on_its_best_row_not_its_first() {
    let rows = [row("OpenCode", "o1", "NOT INSTALLED"), row("OpenCode", "o2", "READY"), row("Codex CLI", "c1", "SIGN IN")];
    assert_eq!(groups(&rows)[0], "OpenCode");
}

#[test]
fn sorts_alphabetically_inside_a_band_the_way_settings_already_does() {
    let rows = [row("Codex CLI", "c", "READY"), row("Claude Code", "a", "READY"), row("OpenCode", "o", "READY")];
    assert_eq!(groups(&rows), ["Claude Code", "Codex CLI", "OpenCode"]);
}

#[test]
fn does_not_move_a_group_out_of_its_band_to_satisfy_the_alphabet() {
    let rows = [row("Apple", "a", "NOT INSTALLED"), row("Zebra", "z", "READY")];
    assert_eq!(groups(&rows), ["Zebra", "Apple"]);
}

#[test]
fn still_sorts_the_active_row_to_the_top_of_its_own_group() {
    let rows = [row("OpenCode", "o1", "READY"), row("OpenCode", "o2", "ACTIVE"), row("Claude Code", "c", "NOT INSTALLED")];
    let keys: Vec<&str> = order_route_rows(&rows, &[]).into_iter().map(|i| rows[i].key.as_str()).collect();
    assert_eq!(keys, ["o2", "o1", "c"]);
}

// --- the-chip-keeps-free.test.ts

#[test]
fn splits_a_trailing_free_off_the_name() {
    assert_eq!(free_tag_of("Muse Spark 1.3 Contributor Free"), ("Muse Spark 1.3 Contributor".to_owned(), true));
    assert_eq!(free_tag_of("Mimo V2.6 Flash Free"), ("Mimo V2.6 Flash".to_owned(), true));
}

#[test]
fn leaves_every_other_name_whole() {
    assert_eq!(free_tag_of("Haiku 4.5"), ("Haiku 4.5".to_owned(), false));
    assert_eq!(free_tag_of("Freehand 2"), ("Freehand 2".to_owned(), false));
    assert_eq!(free_tag_of("Free"), ("Free".to_owned(), false));
}

// --- older-claude-versions-are-folded.test.tsx: the fold's function case

#[test]
fn lets_a_search_reach_into_it_without_opening_it() {
    let fold = |group: &str, label: &str, older: bool| FoldRow { group: group.into(), label: label.into(), older };
    let rows = [
        fold("Claude Code · your account", "Opus 5.5", false),
        fold("Claude Code · your account", "Opus 4.8", true),
        fold("Recent", "Opus 4.8", true),
    ];
    let shown: Vec<String> = unfolded_rows(&rows, false).iter().map(|r| format!("{}/{}", r.group, r.label)).collect();
    assert_eq!(shown, ["Claude Code · your account/Opus 5.5", "Recent/Opus 4.8"]);
    assert_eq!(unfolded_rows(&rows, true).len(), 3);
    let by_group = older_by_group(&rows);
    let held: Vec<&str> = by_group.iter().map(|(g, _)| g.as_str()).collect();
    assert_eq!(held, ["Claude Code · your account"]);
}

// --- a-model-is-one-line.test.tsx: the words of the line and the hover

#[test]
fn keeps_what_the_catalogue_says_and_leaves_the_effort_levels_to_the_hover() {
    let five: Vec<String> = ["low", "medium", "high", "xhigh", "max"].map(String::from).to_vec();
    assert_eq!(row_detail(Some("Always the newest Fable"), None, &five), "Always the newest Fable");
    assert_eq!(
        row_title("Fable 5.1", Some("Always the newest Fable"), None, &five),
        "Fable 5.1 · Always the newest Fable · 5 effort levels · low, medium, high, xhigh, max"
    );
}

#[test]
fn a_model_the_catalogue_says_nothing_about_keeps_its_levels_as_its_line_in_words() {
    let two: Vec<String> = ["low", "high"].map(String::from).to_vec();
    assert_eq!(row_detail(Some(""), None, &two), "Low, High");
    let cursorish: Vec<String> = ["low", "low-fast", "medium", "high", "xhigh", "max", "max-fast"].map(String::from).to_vec();
    assert_eq!(row_detail(Some(""), None, &cursorish), "Low to Max · Fast");
}

// --- a-busy-free-model-offers-the-next.test.tsx: the free model to offer

fn model(id: &str) -> Model {
    Model { id: id.into(), name: id.into(), efforts: Vec::new() }
}

/// Electron's CATALOGUE. Its model of the person's own (`mine/cheap-free`, `own: true`) is Rust's `own-` id.
fn catalogue() -> Vec<Model> {
    ["opencode/big-pickle", "opencode/ling-3.0-flash-fin-free", "opencode/beam-free", "own-cheap-free", "opencode/glm-5.3-flash-free"]
        .map(model)
        .to_vec()
}

fn next(runtime: &str, at: &str, models: &[Model]) -> Option<String> {
    next_free_model(runtime, Some(at), models, &[]).map(|m| m.id.clone())
}

#[test]
fn is_the_next_of_opencodes_free_models_after_the_one_the_run_is_on() {
    assert_eq!(next("opencode", "opencode/glm-5.3-flash-free", &catalogue()).as_deref(), Some("opencode/beam-free"));
}

#[test]
fn skips_a_model_of_the_persons_own_and_goes_round_after_the_last() {
    assert_eq!(next("opencode", "opencode/beam-free", &catalogue()).as_deref(), Some("opencode/ling-3.0-flash-fin-free"));
    assert_eq!(next("opencode", "opencode/ling-3.0-flash-fin-free", &catalogue()).as_deref(), Some("opencode/glm-5.3-flash-free"));
}

#[test]
fn is_nothing_for_a_paid_route_a_model_of_their_own_another_runtime_or_a_lone_free_model() {
    assert_eq!(next("opencode", "opencode/big-pickle", &catalogue()), None);
    assert_eq!(next("opencode", "own-cheap-free", &catalogue()), None);
    assert_eq!(next("claude", "opencode/ling-3.0-flash-fin-free", &catalogue()), None);
    assert_eq!(next("opencode", "opencode/ling-3.0-flash-fin-free", &catalogue()[..2]), None);
}

// --- a-free-model-that-says-nothing-is-offered-another.test.tsx: a free model that has said nothing

/// 2026-11-01T05:00:00.000Z
const BEGAN: i64 = 1_793_509_200_000;

fn event(kind: &str, ms: i64) -> QuietEvent {
    QuietEvent { kind: kind.into(), at_ms: Some(BEGAN + ms), code: None }
}

fn quiet(model: &str, runtime: &str, events: &[QuietEvent], now: i64) -> bool {
    quiet_free_run(Some(runtime), Some(model), events, now)
}

const FREE: &str = "opencode/deepseek-v4.1-flash-free";

#[test]
fn is_quiet_two_minutes_after_its_run_began_with_nothing_back_and_not_a_moment_before() {
    let events = [event("run.started", 0), event("adapter.diagnostic", 5_000)];
    assert!(quiet(FREE, "opencode", &events, BEGAN + QUIET_FREE_MS));
    assert!(!quiet(FREE, "opencode", &events, BEGAN + QUIET_FREE_MS - 1));
}

#[test]
fn is_never_quiet_once_anything_came_back_a_word_a_thought_a_step_or_a_tool() {
    for kind in ["message.delta", "step.started", "tool.started", "plan.updated"] {
        assert!(!quiet(FREE, "opencode", &[event("run.started", 0), event(kind, 90_000)], BEGAN + 10 * QUIET_FREE_MS), "{kind}");
    }
}

#[test]
fn is_only_an_opencode_free_model_and_only_once_its_run_has_begun() {
    let silent = [event("run.started", 0)];
    let later = BEGAN + 10 * QUIET_FREE_MS;
    assert!(!quiet("opencode/claude-opus-5.5", "opencode", &silent, later));
    assert!(!quiet("kilo/kilo-auto/free", "kilo", &silent, later));
    assert!(!quiet_free_run(Some("opencode"), None, &silent, later));
    assert!(!quiet(FREE, "opencode", &[], later));
}

#[test]
fn is_not_quiet_while_its_provider_is_said_to_be_busy() {
    let busy = QuietEvent { code: Some("opencode.provider_busy.runtime_error".into()), ..event("adapter.diagnostic", 30_000) };
    let other = QuietEvent { code: Some("opencode.starting".into()), ..event("adapter.diagnostic", 30_000) };
    assert!(!quiet(FREE, "opencode", &[event("run.started", 0), busy], BEGAN + 10 * QUIET_FREE_MS));
    assert!(quiet(FREE, "opencode", &[event("run.started", 0), other], BEGAN + 10 * QUIET_FREE_MS));
}

#[test]
fn the_flagships_rank_as_status_ts_lists_them() {
    assert_eq!(flagship_rank("AUTO"), Some(0));
    assert_eq!(flagship_rank("claude-opus-5-5"), Some(3));
    assert_eq!(flagship_rank("gpt-5.6-mini"), Some(9));
    assert_eq!(flagship_rank("big-pickle"), None);
}

/// Not an Electron case: Electron's "sinks what does not exist" pair is already in alphabetical order, so a band that
/// put PLANNED level with NOT INSTALLED passed it. Here the alphabet points the other way.
#[test]
fn what_does_not_exist_sinks_below_what_is_not_installed_even_against_the_alphabet() {
    let rows = [row("Apple", "a", "PLANNED"), row("Zebra", "z", "NOT INSTALLED")];
    assert_eq!(groups(&rows), ["Zebra", "Apple"]);
}

/// Not an Electron case of this file: orderRouteRows' in-group score. What was used, most recent first, then the
/// flagships, then the rest as listed.
#[test]
fn inside_a_group_what_was_used_comes_first_then_the_flagships_then_the_rest() {
    let r = |key: &str, model: &str| RouteRow { key: key.into(), group: "Claude Code".into(), model: Some(model.into()), tag: Some("READY".into()) };
    let rows = [r("claude:old", "claude-haiku-4-5"), r("claude:opus", "claude-opus-5-5"), r("claude:used", "claude-sonnet-4-6")];
    let keys: Vec<&str> = order_route_rows(&rows, &["claude:used".to_owned()]).into_iter().map(|i| rows[i].key.as_str()).collect();
    assert_eq!(keys, ["claude:used", "claude:opus", "claude:old"]);
}

/// Not an Electron case of this file: the person's own models lead their band (0.357).
#[test]
fn your_models_lead_their_band() {
    let rows = [row("Claude Code", "c", "READY"), row(OWN_MODELS_GROUP, "o", "READY")];
    assert_eq!(groups(&rows), [OWN_MODELS_GROUP, "Claude Code"]);
}

/// Not an Electron case: Electron's best-row pair lists the best row last, so "the last row decides" passed it.
#[test]
fn takes_a_group_on_its_best_row_wherever_it_is_listed() {
    let rows = [row("OpenCode", "o1", "READY"), row("OpenCode", "o2", "NOT INSTALLED"), row("Codex CLI", "c1", "SIGN IN")];
    assert_eq!(groups(&rows)[0], "OpenCode");
}

// --- a-row-repeats-nothing-its-heading-says.test.tsx

#[test]
fn treats_a_repeated_runtime_name_as_the_heading_and_a_limit_or_an_endpoint_as_news() {
    assert!(row_detail_repeats_heading("Through Antigravity CLI", "Antigravity · your account"));
    assert!(row_detail_repeats_heading("Antigravity model tier", "Antigravity · your account"));
    assert!(row_detail_repeats_heading("Through OpenCode", "OpenCode"));
    assert!(!row_detail_repeats_heading("Free · no sign-in", "OpenCode"));
    assert!(!row_detail_repeats_heading("Your model · llm.acme.example", "Your models"));
    assert!(!row_detail_repeats_heading(
        "Signed in, but the last run on it ended on your account's usage limit: resets Thursday",
        "Codex CLI · your account"
    ));
}

// --- a-turn-that-goes-quiet-says-so.test.tsx

#[test]
fn counts_whole_minutes_from_when_it_was_last_heard_and_only_once_there_are_five() {
    let now: i64 = 1_793_512_800_000;
    let ago = |mins: f64| -> String {
        chrono::DateTime::from_timestamp_millis(now - (mins * 60_000.0) as i64).unwrap().to_rfc3339()
    };
    assert_eq!(quiet_minutes(Some(&ago(4.9)), now), None);
    assert_eq!(quiet_minutes(Some(&ago(5.0)), now), Some(5));
    assert_eq!(quiet_minutes(Some(&ago(42.5)), now), Some(42));
    assert_eq!(quiet_minutes(None, now), None);
    assert_eq!(quiet_minutes(Some("not a time"), now), None);
}

// --- a-context-menu-stays-in-the-window.test.tsx

#[test]
fn at_the_pointer_when_it_fits_there() {
    assert_eq!(place_menu((200.0, 150.0), (220.0, 300.0), (1477.0, 920.0)), (200.0, 150.0));
}

#[test]
fn upward_from_the_pointer_when_it_would_run_off_the_bottom() {
    let placed = place_menu((120.0, 800.0), (220.0, 300.0), (1477.0, 920.0));
    assert_eq!(placed.1, 800.0 - 300.0);
    assert!(placed.1 + 300.0 <= 920.0 - 8.0);
}

#[test]
fn leftward_from_the_pointer_when_it_would_run_off_the_right() {
    assert_eq!(place_menu((1400.0, 150.0), (220.0, 300.0), (1477.0, 920.0)).0, 1400.0 - 220.0);
}

#[test]
fn held_off_the_edge_when_neither_side_has_room() {
    let placed = place_menu((100.0, 160.0), (220.0, 300.0), (500.0, 320.0));
    assert_eq!(placed.1, 320.0 - 8.0 - 300.0);
    assert!(placed.1 >= 8.0);
}
