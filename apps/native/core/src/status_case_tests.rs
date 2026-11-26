//! status.test.ts (apps/desktop/src/renderer/src), one Rust test per case for the cases whose code is in the engine:
//! the recent-routes shortcut, the mode words and menus, every row an agent's status can draw, the receipt's words,
//! the prune preview, the picker's order and the effort levels. Each test's expected words and values are Electron's,
//! from status.test.ts and status.ts. The window's own cases (the picker's cap, route search, the missions list) are in
//! `src/ui/status_case_tests.rs`; the teammate-status cases are in `face_status_tests.rs`.

use crate::agents::{integration_of, row_status, Auth, Found, Integration, Probe, RowStatus};
use crate::effort_levels::{default_effort, effort_after_route_change};
use crate::inspector::{account_phrase, checkpoint_label, sandbox_phrase, short_mission_id};
use crate::mode_facts::{mode_label, modes_for, runs_on, runs_on_platform, unavailable_reason, unavailable_reason_on};
use crate::recent_routes::{shortcut_keys, shortcut_keys_up_to};
use crate::retention::{prune_preview_summary, PruneResult};
use crate::route_rows::{flagship_rank, order_route_rows, RouteRow};

fn keys(list: &[&str]) -> Vec<String> {
    list.iter().map(|k| (*k).to_owned()).collect()
}
fn levels(list: &[&str]) -> Vec<String> {
    keys(list)
}

// ---- the routes a person moves between (recentRouteRows) ----

/// The three rows and the long list of status.test.ts's `rows` and `longList`.
fn rows3() -> Vec<String> {
    keys(&["codex:gpt-5.6-sol", "claude:sonnet", "cursor:composer-2.5"])
}
fn long_list() -> Vec<String> {
    let mut all = rows3();
    all.extend((0..9).map(|i| format!("codex:filler-{i}")));
    all
}

#[test]
fn lifts_the_recently_used_routes_into_a_group_of_their_own_newest_first() {
    // The window heads them "Recent" (thread_view.rs); the keys are the routes, newest first.
    assert_eq!(shortcut_keys(&long_list(), &keys(&["claude:sonnet", "codex:gpt-5.6-sol"])), keys(&["claude:sonnet", "codex:gpt-5.6-sol"]));
}

#[test]
fn withholds_the_group_for_a_single_recent_route_which_is_the_one_you_are_on() {
    assert!(shortcut_keys(&long_list(), &keys(&["claude:sonnet"])).is_empty());
    assert!(shortcut_keys(&long_list(), &[]).is_empty());
}

#[test]
fn withholds_the_group_when_the_whole_list_is_short_enough_to_take_in_at_once() {
    assert!(shortcut_keys(&rows3(), &keys(&["claude:sonnet", "codex:gpt-5.6-sol"])).is_empty());
}

#[test]
fn withholds_the_group_when_it_would_be_half_the_list_or_more() {
    let many: Vec<String> = (0..12).map(|i| format!("codex:model-{i}")).collect();
    assert!(shortcut_keys_up_to(&many, &many[..6], 6).is_empty());
    assert_eq!(shortcut_keys(&many, &many[..3]).len(), 3);
}

#[test]
fn never_resurrects_a_route_the_runtime_has_stopped_offering() {
    let recent = keys(&["codex:retired-model", "claude:sonnet", "cursor:composer-2.5"]);
    assert_eq!(shortcut_keys(&long_list(), &recent), keys(&["claude:sonnet", "cursor:composer-2.5"]));
}

#[test]
fn stops_at_the_limit_rather_than_repeating_the_whole_list() {
    assert_eq!(shortcut_keys_up_to(&long_list(), &rows3(), 2).len(), 2);
}

// ---- the mode a roster card shows ----

#[test]
fn names_the_mode_the_teammate_actually_last_ran_in() {
    assert_eq!(mode_label(Some("accept-edits")), "edit");
    assert_eq!(mode_label(Some("approve-each")), "approve each action");
    assert!(mode_label(Some("ask")).contains("read-only"));
}

#[test]
fn says_a_teammate_who_has_never_run_has_no_mode_yet_rather_than_inventing_one() {
    assert_eq!(mode_label(None), "not set yet");
}

// ---- nothing is live unless discovery proved it (routeRowStatus, routeRowTag, integrationOf) ----

/// status.test.ts's `runtime()`: Codex CLI, installed, signed in, ready.
fn runtime() -> Found {
    Found {
        id: "codex".into(),
        display_name: "Codex CLI".into(),
        installed: true,
        version: Some("0.151.0-alpha.7.2".into()),
        auth: Auth::Authenticated,
        ready: true,
        status: Probe::Ready,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

/// routeRowTag: AT LIMIT outranks ACTIVE; otherwise ACTIVE only on a selectable row.
fn route_row_tag(status: &RowStatus, active: bool) -> &'static str {
    if status.tag == "AT LIMIT" {
        return status.tag;
    }
    if active && status.selectable { "ACTIVE" } else { status.tag }
}

#[test]
fn never_issues_active_or_ready_for_a_runtime_that_is_not_ready() {
    let not_ready = [
        Found { ready: false, status: Probe::ProbeFailed, ..runtime() },
        Found { ready: false, status: Probe::Offline, ..runtime() },
        Found { ready: false, status: Probe::AuthRequired, ..runtime() },
        Found { ready: false, status: Probe::Missing, installed: false, ..runtime() },
        // A failed probe with a stale ready flag: either field alone withholds the label.
        Found { ready: true, status: Probe::ProbeFailed, ..runtime() },
        Found { ready: false, status: Probe::Ready, ..runtime() },
    ];
    for found in not_ready {
        let status = row_status(&found, Integration::Live, true, None);
        assert!(status.tag != "ACTIVE" && status.tag != "READY", "{found:?} -> {}", status.tag);
        assert!(!status.selectable, "{found:?}");
    }
}

#[test]
fn marks_a_ready_runtime_active_only_when_it_is_the_active_one() {
    assert_eq!(row_status(&runtime(), Integration::Live, true, None).tag, "ACTIVE");
    assert_eq!(row_status(&runtime(), Integration::Live, false, None).tag, "READY");
}

#[test]
fn never_calls_a_half_built_adapter_live_even_when_its_runtime_is_ready() {
    let claude = Found { id: "claude".into(), display_name: "Claude Code".into(), ..runtime() };
    let status = row_status(&claude, Integration::Preview, true, None);
    assert_eq!(status.tag, "PREVIEW");
    assert!(status.selectable);
    assert!(status.detail.contains("not been tried through a whole run"), "{}", status.detail);
    assert!(!status.detail.contains("not durable"), "{}", status.detail);
}

#[test]
fn keeps_a_planned_runtime_non_interactive_whatever_discovery_says() {
    let status = row_status(&runtime(), Integration::Planned, true, None);
    assert_eq!(status.tag, "PLANNED");
    assert!(!status.selectable);
}

#[test]
fn separates_sign_in_from_missing_and_from_broken() {
    let tag = |found: Found| row_status(&found, Integration::Live, false, None);
    assert_eq!(tag(Found { installed: false, ready: false, status: Probe::Missing, ..runtime() }).tag, "NOT INSTALLED");
    assert_eq!(tag(Found { ready: false, status: Probe::AuthRequired, auth: Auth::Unauthenticated, ..runtime() }).tag, "SIGN IN");
    assert!(tag(Found { ready: false, status: Probe::Offline, ..runtime() }).detail.contains("could not be reached"));
    assert_eq!(tag(Found { ready: false, status: Probe::ProbeFailed, ..runtime() }).tag, "CHECKING");
    assert_eq!(tag(Found { ready: false, status: Probe::Offline, ..runtime() }).tag, "CHECKING");
    assert!(!tag(Found { ready: false, status: Probe::ProbeFailed, ..runtime() }).selectable);
}

// case: Antigravity through its CLI carries no Experimental tag; through its app it still does (0.541)
#[test]
fn antigravity_through_its_cli_carries_no_experimental_tag_through_its_app_it_still_does() {
    let app = Found { id: "antigravity".into(), display_name: "Antigravity".into(), ..runtime() };
    let cli = Found { through_cli: true, ..app.clone() };
    assert_eq!(row_status(&cli, Integration::Experimental, false, None).tag, "READY");
    assert_eq!(row_status(&app, Integration::Experimental, false, None).tag, "EXPERIMENTAL");
}

#[test]
fn does_not_put_active_on_a_runtime_that_is_not_there() {
    let missing = row_status(&Found { installed: false, ready: false, status: Probe::Missing, ..runtime() }, Integration::Live, true, None);
    assert_eq!(route_row_tag(&missing, true), "NOT INSTALLED");
    let signed_out = row_status(
        &Found { ready: false, status: Probe::AuthRequired, auth: Auth::Unauthenticated, ..runtime() },
        Integration::Live,
        true,
        None,
    );
    assert_eq!(route_row_tag(&signed_out, true), "SIGN IN");
}

// ---- how far each integration goes (RUNTIME_INTEGRATION, integrationOf) ----

#[test]
fn says_live_only_for_the_runtimes_that_can_own_a_mission_today() {
    for id in ["codex", "claude", "cursor"] {
        assert_eq!(integration_of(id), Integration::Live, "{id}");
    }
}

#[test]
fn does_not_claim_a_runtime_the_host_refuses_to_run() {
    assert_eq!(integration_of("gemini"), Integration::Planned);
    assert_eq!(integration_of("omniroute"), Integration::Planned);
}

#[test]
fn treats_a_runtime_it_has_never_heard_of_as_planned_not_as_live() {
    assert_eq!(integration_of("something-new"), Integration::Planned);
}

// ---- mission phase and receipt truth; provenance formatting ----

#[test]
fn shortens_a_real_mission_id_rather_than_inventing_a_counter() {
    assert_eq!(short_mission_id("mission_b806b862-bd4d-4fad-a011-efea6140f46d"), "b806b862");
    assert_eq!(short_mission_id("b806b862-bd4d"), "b806b862");
}

#[test]
fn labels_checkpoints_from_their_1_based_epoch() {
    assert_eq!(checkpoint_label(1), "ck_1");
    assert_eq!(checkpoint_label(14), "ck_14");
}

// ---- what a prune says before it happens ----

fn preview(deleted: usize, continuity: usize, running: usize) -> PruneResult {
    PruneResult {
        deleted: (0..deleted).map(|i| format!("d{i}")).collect(),
        kept_for_continuity: (0..continuity).map(|i| format!("c{i}")).collect(),
        kept_as_running: (0..running).map(|i| format!("r{i}")).collect(),
        ..Default::default()
    }
}

#[test]
fn states_the_count_that_would_go_and_names_everything_held_back() {
    assert_eq!(
        prune_preview_summary(&preview(3, 2, 1)),
        "Delete 3 turns for good, with 2 turns kept as part of a conversation you are keeping and 1 turn kept because they are running."
    );
}

#[test]
fn says_why_nothing_would_go_rather_than_just_saying_nothing() {
    assert_eq!(prune_preview_summary(&preview(0, 2, 0)), "Nothing would be deleted: 2 turns still part of a conversation you are keeping.");
    assert_eq!(prune_preview_summary(&preview(0, 0, 0)), "Nothing is old enough to delete.");
}

#[test]
fn counts_one_mission_as_one() {
    assert_eq!(prune_preview_summary(&preview(1, 0, 0)), "Delete 1 turn for good.");
}

// ---- a read-only mode, where it is real; which modes a route can actually run ----

// case: offers Cursor Ask and Plan on Windows too (0.485: its own ask mode holds)
#[test]
fn offers_cursor_ask_and_plan_on_windows_too() {
    assert!(runs_on_platform("ask", "cursor", Some("win32")));
    assert!(runs_on_platform("plan", "cursor", Some("win32")));
    assert_eq!(modes_for("cursor", Some("win32")), ["ask", "plan", "accept-edits", "auto"]);
    assert_eq!(unavailable_reason_on("ask", "cursor", Some("win32")), None);
}

#[test]
fn and_where_the_sandbox_exists() {
    assert_eq!(modes_for("cursor", Some("darwin")), ["ask", "plan", "accept-edits", "auto"]);
}

#[test]
fn leaves_the_other_runtimes_alone_on_every_platform() {
    assert_eq!(modes_for("codex", Some("win32")), ["ask", "plan", "accept-edits", "approve-each", "auto"]);
    assert_eq!(modes_for("claude", Some("win32")), ["ask", "plan", "accept-edits", "approve-each", "auto"]);
}

#[test]
fn offers_plan_exactly_where_ask_is() {
    assert!(!runs_on("plan", "gemini"));
    assert!(runs_on("plan", "antigravity"));
    assert_eq!(mode_label(Some("plan")), "plan \u{b7} read-only");
}

#[test]
fn keeps_per_action_approvals_to_the_runtime_that_can_stop_and_ask() {
    assert!(runs_on("approve-each", "codex"));
    assert!(!runs_on("approve-each", "cursor"));
    assert!(runs_on("approve-each", "claude"));
}

#[test]
fn lets_every_runtime_read_and_edit() {
    for runtime in ["codex", "claude", "cursor"] {
        assert!(runs_on("ask", runtime), "{runtime}");
        assert!(runs_on("accept-edits", runtime), "{runtime}");
    }
}

#[test]
fn names_the_runtime_when_it_says_no_so_the_menu_can_explain_itself() {
    assert!(unavailable_reason("approve-each", "cursor").unwrap().contains("Cursor Agent"));
    assert_eq!(unavailable_reason("ask", "cursor"), None);
}

// ---- the Auto mode, which has to be switched on before it is offered ----

#[test]
fn is_always_offered_whatever_the_workspace_switch_says() {
    assert!(modes_for("codex", Some("win32")).contains(&"auto"));
}

// case: is offered on every runtime Locust has a handle on, Antigravity (0.540) and Muse (0.773) included
#[test]
fn auto_is_offered_on_every_runtime_locust_has_a_handle_on() {
    for runtime in ["codex", "claude", "cursor", "opencode", "copilot", "antigravity", "muse"] {
        assert!(modes_for(runtime, Some("win32")).contains(&"auto"), "{runtime}");
    }
}

#[test]
fn is_never_the_mode_a_route_falls_back_to() {
    assert_ne!(modes_for("codex", Some("win32"))[0], "auto");
    assert_ne!(modes_for("cursor", Some("win32"))[0], "auto");
}

#[test]
fn has_its_own_phrase_for_what_a_run_was_allowed() {
    assert_eq!(sandbox_phrase(Some("full-access")), "may edit anything on this machine");
    assert_eq!(sandbox_phrase(Some("workspace-write")), "may edit the workspace");
    assert_eq!(sandbox_phrase(Some("read-only")), "read-only");
    assert_eq!(sandbox_phrase(None), "read-only");
}

#[test]
fn names_the_sign_in_a_run_used_in_words_not_by_its_route_id() {
    assert_eq!(account_phrase("codex", "account-default", "codex-account:default"), "your Codex CLI sign-in");
    assert_eq!(account_phrase("antigravity", "flash", "antigravity:hub"), "through the Antigravity app");
    assert_eq!(account_phrase("opencode", "opencode/ling-3.0-flash-fin-free", "opencode-account:default"), "none \u{2014} a free model");
}

#[test]
fn says_what_it_allowed_not_what_it_was_called() {
    assert_eq!(mode_label(Some("auto")), "auto \u{b7} whole machine");
}

// ---- the order rows are offered in; the curated shortlist; the route you are on sorts first ----

fn route(key: &str, group: &str) -> RouteRow {
    RouteRow { key: key.into(), group: group.into(), model: None, tag: None }
}
fn modelled(key: &str, group: &str, model: &str) -> RouteRow {
    RouteRow { key: key.into(), group: group.into(), model: Some(model.into()), tag: None }
}
fn ordered<'a>(rows: &'a [RouteRow], recent: &[&str]) -> Vec<&'a RouteRow> {
    order_route_rows(rows, &keys(recent)).into_iter().map(|i| &rows[i]).collect()
}

#[test]
fn puts_what_this_person_has_run_first_newest_first_inside_its_own_runtime() {
    let rows = [route("cursor:a", "CURSOR"), route("cursor:b", "CURSOR"), route("cursor:c", "CURSOR")];
    let keys: Vec<_> = ordered(&rows, &["cursor:c", "cursor:b"]).iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["cursor:c", "cursor:b", "cursor:a"]);
}

#[test]
fn never_moves_a_row_out_of_its_runtime() {
    let rows = [route("codex:a", "CODEX"), route("cursor:b", "CURSOR"), route("codex:c", "CODEX")];
    let out = ordered(&rows, &["cursor:b", "codex:c"]);
    assert_eq!(out.iter().map(|r| r.group.as_str()).collect::<Vec<_>>(), ["CODEX", "CODEX", "CURSOR"]);
    assert_eq!(out.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), ["codex:c", "codex:a", "cursor:b"]);
}

#[test]
fn leaves_the_runtime_order_alone_when_nothing_has_been_run() {
    let rows = [route("codex:a", "CODEX"), route("cursor:b", "CURSOR")];
    assert_eq!(ordered(&rows, &[]).iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), ["codex:a", "cursor:b"]);
}

#[test]
fn puts_the_flagship_families_above_the_rest_when_nothing_has_been_run() {
    let rows = [
        modelled("cursor:aardvark-1", "CURSOR", "aardvark-1"),
        modelled("cursor:cursor-grok-4.6", "CURSOR", "cursor-grok-4.6"),
        modelled("cursor:zebra-9", "CURSOR", "zebra-9"),
    ];
    let models: Vec<_> = ordered(&rows, &[]).iter().map(|r| r.model.clone().unwrap()).collect();
    assert_eq!(models, ["cursor-grok-4.6", "aardvark-1", "zebra-9"]);
}

#[test]
fn still_puts_what_this_person_ran_above_the_curated_list() {
    let rows = [modelled("cursor:cursor-grok-4.6", "CURSOR", "cursor-grok-4.6"), modelled("cursor:zebra-9", "CURSOR", "zebra-9")];
    let models: Vec<_> = ordered(&rows, &["cursor:zebra-9"]).iter().map(|r| r.model.clone().unwrap()).collect();
    assert_eq!(models, ["zebra-9", "cursor-grok-4.6"]);
}

#[test]
fn puts_a_newer_flagship_family_above_the_one_it_replaces() {
    let astra = flagship_rank("gpt-6-astra").expect("GPT-6 is curated");
    let sol = flagship_rank("gpt-5.6-sol").expect("GPT-5.6 is curated");
    assert!(astra < sol, "{astra} < {sol}");
}

#[test]
fn hides_nothing_a_model_on_no_list_is_still_offered() {
    let rows = [modelled("cursor:obscure", "CURSOR", "obscure")];
    assert_eq!(ordered(&rows, &[]).len(), 1);
    assert_eq!(flagship_rank("obscure"), None);
}

fn active_first_row(key: &str, tag: Option<&str>) -> RouteRow {
    RouteRow { key: key.into(), group: "CODEX CLI".into(), model: Some(key.into()), tag: tag.map(str::to_owned) }
}

#[test]
fn puts_the_active_row_above_flagships_and_everything_else() {
    let rows = [active_first_row("gpt-6-astra", None), active_first_row("gpt-5.6-sol", None), active_first_row("account-default", Some("ACTIVE"))];
    assert_eq!(ordered(&rows, &[])[0].key, "account-default");
}

#[test]
fn still_beats_a_recently_used_row_because_it_is_the_one_in_use_now() {
    let rows = [active_first_row("gpt-5.6-sol", None), active_first_row("account-default", Some("ACTIVE"))];
    assert_eq!(ordered(&rows, &["gpt-5.6-sol"])[0].key, "account-default");
}

#[test]
fn leaves_the_order_alone_when_nothing_is_active() {
    let rows = [active_first_row("gpt-6-astra", None), active_first_row("gpt-5.6-sol", None)];
    assert_eq!(ordered(&rows, &["gpt-5.6-sol"])[0].key, "gpt-5.6-sol");
}

// ---- the words for what a teammate does (roleLabelOf) ----

// case: uses a Custom teammate's own title, and the role name for everyone else
#[test]
fn uses_a_custom_teammates_own_title_and_the_role_name_for_everyone_else() {
    use crate::routine_templates::ProposableTeammate;
    let mate = |role: &str, title: Option<&str>| ProposableTeammate {
        teammate_id: "tm".into(),
        role: role.into(),
        role_title: title.map(str::to_owned),
        runtime: None,
    };
    assert_eq!(mate("Custom", Some("Release manager")).role_label(), "Release manager");
    assert_eq!(mate("Custom", None).role_label(), "Custom");
    assert_eq!(mate("Custom", Some("   ")).role_label(), "Custom");
    assert_eq!(mate("Docs & QA", Some("ignored")).role_label(), "Docs & QA");
}

// ---- effort reads as a constant ----

#[test]
fn prefers_medium_the_ordinary_setting_on_every_runtime_here() {
    assert_eq!(default_effort(&levels(&["low", "medium", "high", "xhigh", "max"]), None).as_deref(), Some("medium"));
}

#[test]
fn takes_the_middle_when_there_is_no_medium() {
    assert_eq!(default_effort(&levels(&["low", "high", "max"]), None).as_deref(), Some("high"));
}

#[test]
fn biases_upward_on_a_two_level_model_as_picking_medium_out_of_five_does() {
    assert_eq!(default_effort(&levels(&["low", "high"]), None).as_deref(), Some("high"));
}

#[test]
fn has_nothing_to_show_when_the_model_reports_no_levels() {
    assert_eq!(default_effort(&[], None), None);
}

#[test]
fn carries_a_chosen_level_across_a_model_switch_that_supports_it() {
    assert_eq!(effort_after_route_change(Some("xhigh"), &levels(&["low", "medium", "xhigh"]), None).as_deref(), Some("xhigh"));
}

#[test]
fn falls_back_to_the_new_model_default_rather_than_to_nothing() {
    assert_eq!(effort_after_route_change(Some("ultra"), &levels(&["low", "medium", "high"]), None).as_deref(), Some("medium"));
}

#[test]
fn still_shows_nothing_when_the_new_model_reports_no_levels() {
    assert_eq!(effort_after_route_change(Some("high"), &[], None), None);
}

#[test]
fn start_a_new_route_on_the_level_the_runtime_names_as_its_default_where_it_names_one() {
    assert_eq!(default_effort(&levels(&["low", "high", "max"]), Some("max")).as_deref(), Some("max"));
    assert_eq!(default_effort(&levels(&["high", "max"]), Some("high")).as_deref(), Some("high"));
    assert_eq!(default_effort(&levels(&["high", "max"]), None).as_deref(), Some("max"));
    assert_eq!(default_effort(&levels(&["low", "medium", "high"]), Some("ultra")).as_deref(), Some("medium"));
    assert_eq!(effort_after_route_change(Some("xhigh"), &levels(&["high", "max"]), Some("high")).as_deref(), Some("high"));
    assert_eq!(effort_after_route_change(Some("max"), &levels(&["high", "max"]), Some("high")).as_deref(), Some("max"));
}

