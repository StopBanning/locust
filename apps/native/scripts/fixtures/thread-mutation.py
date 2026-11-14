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
name = sys.argv[1]
file, old, new, test = controls[name]
p = pathlib.Path(file)
original = p.read_text()
assert original.count(old) == 1, (name, original.count(old))
log = pathlib.Path('notes/thread-mutation-' + name + '.log')
try:
    p.write_text(original.replace(old, new, 1))
    with log.open('w') as out:
        result = subprocess.run(['timeout', '590', 'python3', 'scripts/gentle-cargo.py', '--timeout', '580', 'test', '--offline', '--target-dir', 'target/thread-controls', '-p', 'locust-core'], stdout=out, stderr=subprocess.STDOUT, env=dict(os.environ, CARGO_PROFILE_DEV_OPT_LEVEL='1', CARGO_PROFILE_DEV_DEBUG='0', RUST_TEST_THREADS='4'))
    output = log.read_text()
    assert result.returncode == 101 and ('::' + test + ' ... FAILED') in output and 'test result: FAILED.' in output, (name, result.returncode, output[-3000:])
    print(name + ': named assertion failed across whole core suite')
finally:
    p.write_text(original)
    with log.open('a') as diff:
        subprocess.run(['git', 'diff'], stdout=diff, check=True)
    subprocess.run(['git', 'diff', '--check'], check=True)
