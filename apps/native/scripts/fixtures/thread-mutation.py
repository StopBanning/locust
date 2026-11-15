#!/usr/bin/env python3
"""One restored source mutation against the entire core suite. Run from apps/native."""
import os, pathlib, subprocess, sys
os.environ["LOCUST_TEST_TS_READER"] = str(pathlib.Path("dist/thread-7-ts-reader/index.js").resolve())
controls = {
    'cursor-mirror': ('core/src/path_words.rs', 'return mirrored;', 'return path.to_owned();', 'reads_the_mirrored_path_as_the_file_you_would_open_yourself'),
    'cursor-project': ('core/src/path_words.rs', '(c[2].to_lowercase() == flattened.to_lowercase())', '(c[2].to_lowercase() != flattened.to_lowercase())', 'leaves_another_projects_mirror_whole_because_there_the_location_is_the_point'),
    'disk-merge': ('core/src/ledger.rs', 'if kind == "tool.started" && p["toolKind"] == "observed_edit"', 'if false && p["toolKind"] == "observed_edit"', 'attaches_the_patch_to_the_row_the_runtime_drew_and_draws_a_row_only_for_a_path_it_never_named'),
    'disk-net': ('core/src/activity_entries.rs', '!observed.contains(&path_key(&f.file.path, workspace))', 'true', 'supersedes_every_other_edit_row_for_that_file_so_the_card_counts_the_file_once'),

    'waiting-origin': ('core/src/thread_view.rs', 'if started_at.is_empty() {', 'if true {', 'counts_from_the_start_of_the_turn_so_it_climbs_instead_of_looping'),
    'waiting-first': ('core/src/thread_view.rs', 'events.first()?.get("occurredAt")?.as_str()?', '""', 'falls_back_to_the_first_event_when_the_turn_start_is_unknown'),
    'decision-readonly': ('core/src/decision_standing.rs', 'sandbox == Some("read-only")', 'sandbox == Some("workspace-write")', 'claims_nothing_changed_only_when_the_run_could_not_write'),
    'decision-patch': ('core/src/decision_standing.rs', 'e["payload"].get("patch").is_some()', 'false', 'says_work_is_kept_when_a_patch_actually_came_back'),
    'relay-kind': ('core/src/relay_title.rs', 'kind != Some("relay")', 'false', 'leaves_a_mission_a_person_started_alone'),
    'relay-received': ('core/src/relay_title.rs', 'm.direction == "received"', 'm.direction == "posted"', 'is_named_by_the_message_that_caused_it_never_by_the_host_briefing'),
    'relay-flat': ('core/src/relay_title.rs', '.split_whitespace().collect::<Vec<_>>().join(" ")', '.to_owned()', 'keeps_the_title_to_one_line'),
    'relay-typed': ('core/src/relay_title.rs', 'title(source).unwrap_or_else(|| words.to_owned())', 'words.to_owned()', 'does_not_let_typed_prompt_hand_back_the_briefing_either'),
    'alias-result': ('core/src/model_alias.rs', 'completed.and_then(|e| e["payload"]["resolvedModel"].as_str())', 'completed.and_then(|_| None)', 'learns_it_from_the_result_which_is_the_only_record_that_states_it'),
    'recent-latest': ('core/src/recent_routes.rs', 'held.1.max(at)', 'held.1.min(at)', 'lists_each_route_once_newest_first'),
    'recent-once': ('core/src/recent_routes.rs', 'positions.get(&key)', 'positions.get(&key).filter(|_|false)', 'lists_each_route_once_newest_first'),
    'recent-order': ('core/src/recent_routes.rs', 'right.1.cmp(&left.1)', 'left.1.cmp(&right.1)', 'lists_each_route_once_newest_first'),
    'marker-gap': ('core/src/time_marker.rs', 'if gap < QUIET_GAP_MINUTES', 'if gap < 0 * QUIET_GAP_MINUTES', 'says_nothing_when_turns_follow_each_other_closely'),
    'marker-origin': ('core/src/time_marker.rs', 'let minutes_in = minutes_between(origin, next);', 'let minutes_in = minutes_between(previous, next);', 'measures_elapsed_time_from_the_mission_first_event_not_the_previous_turn'),
    'session-empty': ('core/src/runtime_terminal.rs', '.filter(|id| !id.is_empty())', '.filter(|_| true)', 'ignores_an_empty_session_id_rather_than_treating_it_as_one'),
    'turn-followup': ('core/src/conversation_turns.rs', 'current.continues_reason.as_deref() != Some("route-switch")', 'current.parent.is_none()', 'shows_a_reply_the_words_that_were_typed_for_it_not_the_opening_line'),
    'turn-orphan': ('core/src/conversation_turns.rs', 'else { return (current, &current.prompt); };\n        if !seen.insert', 'else { return (current, ""); };\n        if !seen.insert', 'keeps_the_mission_own_words_when_the_one_it_continues_is_gone'),
    'alias-decode': ('core/src/ledger.rs', 'if is_start && let Some(model)', 'if !is_start && let Some(model)', 'recent_history_learns_the_runtime_model_from_the_ledger'),
    'alias-start': ('core/src/model_alias.rs', 'e["payload"]["evidence"]["raw"]["model"]', 'e["payload"]["evidence"]["raw"]["missing-model"]', 'learns_the_real_name_the_runtime_reported_for_an_alias'),
    'alias-newest': ('core/src/model_alias.rs', 'at > *old', 'true', 'prefers_the_newest_mission_so_a_new_release_replaces_an_old_name'),
    'alias-same': ('core/src/model_alias.rs', '*name != requested', 'requested.is_empty() || true', 'says_nothing_about_an_alias_nobody_has_run_or_one_that_taught_it_nothing'),
    'alias-ledger': ('core/src/ledger.rs', 'reported_model: crate::model_alias::reported_model(&events.values().copied().cloned().collect::<Vec<_>>(), &m.model)', 'reported_model: None', 'recent_history_learns_the_runtime_model_from_the_ledger'),

    'handoff-decode': ('core/src/ledger.rs', '"mission.checkpoint" => {', '"discarded.checkpoint" => {', 'rebuilds_the_divider_from_the_route_switch_checkpoint_it_resumed_from'),
    'notice-ended': ('core/src/ledger.rs', 'if !self.running && !items.iter().any(|i| matches!(i, Item::Tool { .. } | Item::Thought { .. })) {', 'if false {', 'says_a_notice_raised_before_any_work_once_the_turn_ends_with_no_fold_to_put_it_under'),
    'notice-retry': ('core/src/thread_view.rs', '"Trying the model again"', '"Starting"', 'shows_a_rate_limited_opencode_run_retrying_before_anything_has_run'),
    'step-kind': ('core/src/thread_view.rs', 'kind == "reasoning" || item.contains("reasoning")', 'false || item.contains("reasoning")', 'carries_the_step_kind_so_thought_and_action_draw_differently'),
    'handoff-checkpoint': ('core/src/conversation_turns.rs', 'unsettled_count:m.checkpoint_epoch.and_then(|epoch|prior.checkpoint_unsettled.get(&epoch).copied()).unwrap_or(0)', 'unsettled_count:0', 'rebuilds_the_divider_from_the_route_switch_checkpoint_it_resumed_from'),
    'handoff-fold': ('core/src/ledger.rs', 'p["unsettledActions"].as_array().map_or(0, Vec::len)', '0', 'rebuilds_the_divider_from_the_route_switch_checkpoint_it_resumed_from'),
    'peer-hide': ('core/src/protocol_tags.rs', 'let s = crate::peer_share::strip_share_blocks(text);', 'let s = text.to_owned();', 'hides_a_share_block_from_the_agent_bubble_keeping_the_prose'),
    'peer-sort': ('core/src/peer_thread.rs', 'messages.sort_by(|a,b| a.posted_at.cmp(&b.posted_at));', 'messages.sort_by(|a,b| b.posted_at.cmp(&a.posted_at));', 'groups_an_exchange_by_the_other_party_and_marks_whether_anything_was_received'),
    'helper-unnamed': ('core/src/activity_entries.rs', '_ => "a subagent, unnamed",', '_ => "Task",', 'a_helper_the_runtime_never_named_is_said_to_be_unnamed_not_drawn_as_a_path'),
    'helper-kind': ('core/src/ledger.rs', '"task" | "agent" | "subagent" | "spawn_agent"', '"agent" | "subagent" | "spawn_agent"', 'is_its_own_row_what_it_was_asked_and_whether_it_reported_back'),
    'plan-content': ('core/src/ledger.rs', '["step", "text", "title", "name", "content"]', '["step", "text", "title", "name", "missing-content"]', 'reads_the_steps_their_words_and_their_states'),
    'plan-state': ('core/src/ledger.rs', 'status.contains("complete") || status == "done"', 'false', 'reads_the_steps_their_words_and_their_states'),
    'plan-own': ('core/src/ledger.rs', 'steps: read_plan(&p["plan"]),', 'steps: Vec::new(),', 'stands_on_its_own_even_when_the_run_also_did_something'),
    'peer-only-reply': ('core/src/ledger.rs', '&& !self.peer_links.iter().any(|p| p.direction == "posted")', '&& true', 'is_not_reported_as_a_turn_that_said_nothing'),
    'late-target': ('core/src/ledger.rs', '} else if summary.is_empty() {', '} else if false {', 'takes_the_target_from_the_completion_when_the_start_had_none'),
    'late-keep': ('core/src/ledger.rs', '} else if summary.is_empty() {', '} else if true {', 'leaves_a_row_the_start_already_named_alone'),

    'changed-count': ('core/src/activity_entries.rs', 'collect::<std::collections::HashSet<_>>().len()', 'collect::<Vec<_>>().len()', 'counts_two_edits_to_the_same_path_once'),
    'changed-refused': ('core/src/activity_entries.rs', 'crate::activity_summary::is_edit(item) && *status != Status::Failed', 'crate::activity_summary::is_edit(item)', 'does_not_count_a_refused_write_as_a_changed_file'),
    'changed-patch': ('core/src/activity_entries.rs', 'if !files.is_empty() { return files.into_iter()', 'if false { return files.into_iter()', 'counts_one_copilot_multiline_patch_as_one_changed_file'),
    'typed-marker': ('core/src/conversation_turns.rs', 'prompt.rsplit_once(INSTRUCTION_SECTION)', 'prompt.rsplit_once("The person now asks:")', 'does_not_read_the_marker_out_of_a_briefing_that_quotes_the_original_task'),
    'typed-prior': ('core/src/conversation_turns.rs', 'current = prior;', 'return (current, &current.prompt);', 'still_shows_the_original_words_when_the_switch_carried_no_new_instruction'),
    'start-day': ('core/src/time_marker.rs', 'if at.date() == now.date() {', 'if true {', 'carries_the_date_once_the_mission_is_not_from_today'),
    'start-year': ('core/src/time_marker.rs', 'at.year() == now.year()', 'true', 'adds_the_year_only_when_the_mission_is_from_another_one'),
    'start-cache': ('core/src/ledger.rs', 'if index == 0 {\n                snapshot.started_at', 'if index == usize::MAX {\n                snapshot.started_at', 'mission_start_label_is_cached_from_the_original_turn_even_after_a_handoff'),
    'assistant-append': ('core/src/ledger.rs', 'let left = MAX_ASSISTANT.saturating_sub(*used);', 'text.clear(); let left = MAX_ASSISTANT.saturating_sub(*used);', 'rebuilds_an_appended_stream_instead_of_keeping_the_last_fragment'),
    'assistant-replace': ('core/src/ledger.rs', 'if string(p, "operation") == "replace" {', 'if false {', 'honors_a_replace_as_a_replace'),
    'assistant-order': ('core/src/ledger.rs', 'messages.entry(id.clone()).or_insert_with(|| {', 'messages.entry("all".into()).or_insert_with(|| {', 'keeps_separate_messages_separate_and_in_order'),
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

    'rail-tool': ('core/src/signal_rail.rs', 'format!("{name} · {target}")', 'name.to_owned()', 'names_a_call_it_does_not_recognise_by_its_own_name_never_a_guess'),
    'rail-words': ('core/src/signal_rail.rs', 's.replace([\'-\', \'_\'], " ").trim().into()', 's.to_owned()', 'says_what_happened_in_words_not_the_ledgers_event_names'),
    'rail-order': ('core/src/signal_rail.rs', 'rows.reverse();', '/* oldest first */', 'shows_newest_first'),
    'rail-live': ('core/src/signal_rail.rs', 'tone = SignalTone::Live;', 'tone = SignalTone::Muted;', 'marks_a_tool_live_only_while_it_is_open_and_the_run_is_going'),
    'rail-tone': ('core/src/signal_rail.rs', 'tone = SignalTone::Blue;', 'tone = SignalTone::Muted;', 'colours_by_meaning_not_decoration'),
    'rail-plan': ('core/src/signal_rail.rs', 's.state == "done"', 's.state == "pending"', 'says_how_far_the_plan_had_got_not_only_that_it_moved_0_360'),
    'rail-once': ('core/src/signal_rail.rs', 'ty != "tool.started" && started.contains(id)', 'false && started.contains(id)', 'says_each_call_as_a_sentence_one_row_a_call_with_how_it_ended'),
    'rail-helper': ('core/src/signal_rail.rs', 'if p["parentItemId"].as_str()', 'if p["missingParent"].as_str()', 'helper_calls_belong_to_their_activity_row_and_refused_commands_never_ran'),
    'rail-cache': ('core/src/ledger.rs', 'self.signal_rail = crate::signal_rail::build(&event_vec, self.running);', 'self.signal_rail.clear();', 'the_ledger_caches_local_rail_clocks_before_the_window_draws'),
    'peer-flat': ('core/src/peer_thread.rs', 'text?.split_whitespace().collect::<Vec<_>>().join(" ")', 'text?.trim().to_owned()', 'flattens_a_multi_line_message_to_one_line'),
    'peer-open': ('core/src/peer_thread.rs', 'message_count <= 2', 'message_count <= 4', 'leaves_a_long_back_and_forth_collapsed'),
    'peer-empty': ('core/src/peer_thread.rs', 'message_count > 0 &&', 'true &&', 'has_nothing_to_open_when_there_are_no_messages'),
    'peer-snippet': ('core/src/peer_thread.rs', 'format!("{}…", flat.chars()', 'format!("{}", flat.chars()', 'cuts_a_long_message_rather_than_letting_the_card_wrap'),

    'peer-turns': ('core/src/peer_thread.rs', 'turns.iter().enumerate()', 'turns.iter().enumerate().skip(1)', 'draws_the_message_an_earlier_turn_sent_not_only_the_reply'),
    'peer-placement': ('core/src/peer_thread.rs', 'messages.iter().any(|m| m.direction == "received")', 'false', 'puts_what_a_turn_sent_after_its_work_and_what_it_was_handed_before_it'),
    'peer-key': ('core/src/peer_thread.rs', 'format!("peer_{turn_index}_{name}")', 'format!("peer_{name}")', 'gives_the_same_peer_a_distinct_key_on_each_turn'),

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
        result = subprocess.run(['timeout', '590', 'python3', 'scripts/gentle-cargo.py', '--timeout', '570', 'test', '--release', '--offline', '-p', 'locust-core'], stdout=out, stderr=subprocess.STDOUT, env=dict(os.environ, CARGO_PROFILE_DEV_OPT_LEVEL='1', CARGO_PROFILE_DEV_DEBUG='0', RUST_TEST_THREADS='4'))
    output = log.read_text()
    assert result.returncode == 101 and ('::' + test + ' ... FAILED') in output and 'test result: FAILED.' in output, (name, result.returncode, output[-3000:])
    print(name + ': named assertion failed across whole core suite')
finally:
    p.write_text(original)
    with log.open('a') as diff:
        subprocess.run(['git', 'diff'], stdout=diff, check=True)
    subprocess.run(['git', 'diff', '--check'], check=True)
