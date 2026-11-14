#!/usr/bin/env python3
"""One restored source mutation against the entire core suite. Run from apps/native."""
import os, pathlib, subprocess, sys
controls = {
    'limit-item': ('core/src/ledger.rs', 'items.push(Item::Limit { kind: string(p, "kind"), text });', 'items.push(Item::Note { level: "warning".into(), text });', 'surfaces_provider_limits_and_diagnostics_as_their_own_items'),
    'temporary-limit': ('core/src/ledger.rs', 'if !finished && p["kind"] != "temporary-rate-limit" {', 'if !finished {', 'surfaces_provider_limits_and_diagnostics_as_their_own_items'),
    'live-cache': ('core/src/ledger.rs', 'live_line(&event_vec, self.running,', 'live_line(&event_vec, !self.running,', 'the_ledger_caches_the_live_projection_when_events_change'),
    'caret': ('core/src/thread_view.rs', 'running && !final_', '!final_', 'shows_a_caret_only_while_text_is_genuinely_still_arriving'),
    'ended-line': ('core/src/thread_view.rs', 'running && !awaiting_decision', '!awaiting_decision', 'draws_no_live_line_once_the_run_is_over'),
    'approval': ('core/src/thread_view.rs', 'running && !awaiting_decision', 'running || awaiting_decision', 'says_nothing_above_an_approval_even_with_a_tool_open_the_tool_is_what_is_waiting'),
    'step-ended': ('core/src/thread_view.rs', 'step = None;', 'step = step.take();', 'drops_the_live_step_once_it_completes'),
    'writing': ('core/src/thread_view.rs', '("Writing".into(), "writing".into(), false)', '("Working".into(), "working".into(), true)', 'says_it_is_writing_while_the_reply_arrives_on_the_turn_clock_0_581'),
    'host-prompt': ('core/src/thread_view.rs', 'kind.is_none_or(|k|', 'kind.is_none_or(|k| k == "relay" ||', 'never_draws_a_turn_the_host_briefed_as_the_persons_own_words'),
    'peer-target': ('core/src/thread_view.rs', 'link.direction == "received"', 'link.direction == "posted"', 'finds_the_run_a_peer_message_was_delivered_into_so_the_exchange_can_be_opened'),
    'silent': ('core/src/ledger.rs', 'This turn ended without a reply:', 'This turn ended:', 'says_so_when_a_turn_finished_and_wrote_nothing_back'),
    'run-failure': ('core/src/ledger.rs', 'if !crate::thread_view::error_already_shown(&items, &said) {', 'if !said.is_empty() {', 'the_run_failure_does_not_repeat_the_ending_quota_card'),
    'quota': ('core/src/ledger.rs', 'e["payload"]["kind"] == "quota-exhausted"', 'e["payload"]["kind"] == "never-a-limit"', 'says_a_quota_failure_once_not_as_a_card_and_again_as_a_red_line'),
    'edit-class': ('core/src/activity_summary.rs', 'matches!(item, Item::Tool {', 'false && matches!(item, Item::Tool {', 'draws_one_row_per_file_a_runtime_named_without_a_diff_never_one_row_named_after_the_tool'),
    'failed-edit': ('core/src/activity_summary.rs', '*status != Status::Failed', '*status != Status::Running', 'singularizes_honestly'),
    'file-rows': ('core/src/activity_summary.rs', 'if !name.contains(\'\\n\') {', 'if name.contains(\'\\n\') {', 'draws_one_row_per_file_a_runtime_named_without_a_diff_never_one_row_named_after_the_tool'),
    'nonzero': ('core/src/command_outcomes.rs', 'detail.exit_code.is_some_and(|c| c != 0)', 'detail.exit_code.is_some_and(|c| c > 1)', 'counts_a_non_zero_exit_and_says_so_in_amber'),
    'unreported': ('core/src/command_outcomes.rs', 'if finished && *status == Status::Running {', 'if !finished && *status == Status::Running {', 'does_not_count_an_unfinished_turns_open_command_as_unreported'),
    'command-head': ('core/src/command_outcomes.rs', '(?:cd|pushd|chdir|set-location|sl)', '(?:pushd|chdir|set-location|sl)', 'reads_past_claude_codes_cd_in_front_of_a_command'),
    'command-kinds': ('core/src/command_outcomes.rs', '!heads.contains(&head)', 'head.len() > 0', 'names_each_kind_once_so_twelve_commands_of_two_kinds_read_as_twelve'),
}
controls.update({
    'ended-question-gate': ('core/src/thread_view.rs', 'if running || !latest_turn', 'if running && !latest_turn', 'is_not_offered_while_the_run_is_still_going'),
    'ended-question-last': ('core/src/thread_view.rs', 'let last_final = items.iter().rev()', 'let last_final = items.iter()', 'reads_the_last_final_message_not_an_earlier_one'),
    'path-workspace': ('core/src/activity_entries.rs', 'relative_path(path, workspace)', 'relative_path(path, workspace.filter(|_| false))', 'counts_a_relative_and_an_absolute_spelling_as_one'),
    'path-restatement': ('core/src/activity_entries.rs', 'seen.insert(signature)', 'seen.insert(signature) || true', 'counts_one_edit_reported_twice_as_one'),
    'path-hunks': ('core/src/activity_entries.rs', 'f.file.hunks);', 'f.file.hunks.len());', 'still_counts_two_real_edits_to_one_file_as_two'),
    'no-change-permission': ('core/src/activity_trace.rs', 'may_edit==Some(true)', 'may_edit!=Some(false)', 'says_nothing_when_the_caller_does_not_know_what_the_run_was_allowed'),
    'no-change-overlap': ('core/src/activity_trace.rs', 'e["payload"]["code"]=="host.shared_workspace"', 'e["payload"]["code"]=="missing"', 'is_withheld_when_the_host_has_said_it_cannot_tell_what_changed'),
    'cancellation-planned': ('core/src/cancellation.rs', 'latest.map_or((0, 0, planned_steps),', 'latest.map_or((0, 0, 0),', 'separates_tool_outcomes_and_never_substitutes_them_for_unreported_plan_states'),
    'cancellation-dedup': ('core/src/cancellation.rs', '!seen.insert(key(name))', '(!seen.insert(key(name)) && name.is_empty())', 'counts_one_file_once_however_each_tool_spelled_its_path'),

    'conversation-draw': ('core/src/ledger.rs', '(!stitched || !matches!(item, Item::User(_)))', '(true || !matches!(item, Item::User(_)))', 'the_session_stitches_a_midrun_handoff_but_preserves_a_reply_switch'),

    'conversation-walk': ('core/src/conversation_turns.rs', 'turns.reverse();turns', 'turns', 'walks_a_reply_back_to_every_earlier_turn_oldest_first'),
    'conversation-reply': ('core/src/conversation_turns.rs', '&& handoff_instruction(&m.prompt).is_some()', '&& false', 'walks_a_reply_sent_to_another_runtime_back_to_the_turns_before_it_and_marks_the_seam'),
    'conversation-midrun': ('core/src/conversation_turns.rs', 'let mut head=latest;while', 'let mut head=latest;if false {} while false &&', 'keeps_the_turns_before_a_running_mission_that_was_handed_over'),

    'trace-duration': ('core/src/activity_trace.rs', 'crate::ledger::duration_text(ms)', 'crate::ledger::duration_text(0)', 'leads_with_the_duration_and_ends_with_the_exceptions_a_zero_segment_is_absent'),
    'trace-thought': ('core/src/activity_trace.rs', 'if thought>=1000', 'if thought>7000', 'does_not_count_thinking_as_a_tool_call_on_the_line_a_person_reads'),
    'trace-running': ('core/src/activity_trace.rs', 'if outcome=="running" && *shells[0].1==Status::Running', 'if false && *shells[0].1==Status::Running', 'says_the_one_command_is_running_while_it_has_no_result_not_that_it_ran_0_378'),
    'trace-helper': ('core/src/activity_trace.rs', 'if outcome=="running"{0}', 'if true{0}', 'says_a_subagent_did_not_report_in_amber_only_once_the_turn_is_over'),
    'trace-source': ('core/src/activity_trace.rs', 'source:e["sourceAdapter"]', 'source:e["missingSource"]', 'carries_the_notices_it_used_to_only_count_as_sentences_on_the_fold'),

    'live-connector': ('core/src/thread_view.rs', 'crate::mcp_names::mcp_tool_parts(s("name"))', 'crate::mcp_names::mcp_tool_parts("")', 'reads_a_connector_name_that_carries_no_mcp_prefix'),
    'live-detail': ('core/src/thread_view.rs', 'mcp.map(|m| m.server)', 'mcp.map(|_| String::new())', 'names_a_connector_as_a_connector_because_it_reaches_off_this_machine'),
    'exit-result': ('core/src/ledger.rs', 'if code != 0 { *status = Status::Failed; }', 'if code > 1 { *status = Status::Failed; }', 'carries_a_command_row_with_its_exit_result'),
    'patch-rows': ('core/src/activity_entries.rs', 'changes::parse(&patch.text)', 'changes::parse("")', 'turns_one_patch_into_a_row_per_file_it_touched'),
    'patch-counts': ('core/src/activity_entries.rs', 'let counts = file.counts();', 'let counts = (patch.added, patch.removed);', 'sums_the_card_total_from_the_rows_it_will_actually_draw'),
    'patch-reported': ('core/src/activity_entries.rs', 'patch.truncated && single', 'patch.truncated', 'keeps_the_runtime_total_beside_a_truncated_single_file_patch_and_withholds_it_across_several'),
    'patch-large': ('core/src/activity_entries.rs', 'total.0 + total.1 > 200', 'total.0 + total.1 > 400', 'opens_the_first_file_unless_opening_it_would_bury_everything_after_it'),
    'patch-page': ('core/src/activity_entries.rs', '!first.large || page', '!first.large && page', 'opens_a_new_web_page_however_long_it_is_because_it_is_shown_running_0_446'),
    'patch-missing': ('core/src/activity_entries.rs', '&& files(item).is_empty()', '&& !files(item).is_empty()', 'keeps_an_edit_whose_runtime_reported_no_patch_as_a_row_that_says_so'),
    'patch-too-large': ('core/src/activity_entries.rs', 'detail.runtime_status == "result too large to keep"', 'false', 'says_an_edit_whose_result_was_too_large_to_keep_rather_than_that_nobody_confirmed_it'),
    'tool-name': ('core/src/activity_entries.rs', '(name != target)', '(name == target)', 'names_the_tool_on_a_row_whose_target_is_the_same_path_it_edited'),
    'patch-ledger': ('core/src/ledger.rs', 'if let Some(patch) = p.get("patch") {', 'if let Some(patch) = p.get("no-patch") {', 'attaches_a_completion_patch_to_the_tool_that_opened_and_names_the_runtime_that_reported_it'),
})
# A killed process never reaches finally: refuse any inherited control before touching a file.
for control, (file, old, new, test) in controls.items():
    assert pathlib.Path(file).read_text().count(old) == 1, ('unrestored or stale control', control)
name = sys.argv[1]
file, old, new, test = controls[name]
p = pathlib.Path(file)
original = p.read_text()
assert original.count(old) == 1, (name, original.count(old))
log = pathlib.Path('notes/thread-mutation-' + name + '.log')
try:
    p.write_text(original.replace(old, new, 1))
    with log.open('w') as out:
        result = subprocess.run(['timeout', '590', 'python3', 'scripts/gentle-cargo.py', '--timeout', '580', 'test', '--offline', '-p', 'locust-core'], stdout=out, stderr=subprocess.STDOUT, env=dict(os.environ, CARGO_PROFILE_DEV_OPT_LEVEL='1', CARGO_PROFILE_DEV_DEBUG='0', RUST_TEST_THREADS='4'))
    output = log.read_text()
    assert result.returncode == 101 and ('::' + test + ' ... FAILED') in output and 'test result: FAILED.' in output, (name, result.returncode, output[-3000:])
    print(name + ': named assertion failed across whole core suite')
finally:
    p.write_text(original)
    with log.open('a') as diff:
        subprocess.run(['git', 'diff'], stdout=diff, check=True)
    subprocess.run(['git', 'diff', '--check'], check=True)
