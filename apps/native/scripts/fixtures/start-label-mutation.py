#!/usr/bin/env python3
"""Check one start-label window regression headlessly, restoring before exit."""
import os
import pathlib
import subprocess
import sys
os.environ["LOCUST_TEST_TS_READER"] = str(pathlib.Path("dist/thread-7-ts-reader/index.js").resolve())
controls = {
    'relay-history': ('src/main.rs', 'row.title = title.clone();', 'row.title = "Host briefing".into();', 'a_relay_history_refresh_names_the_received_message_in_the_sidebar'),
    'recent-cache': ('src/main.rs', 's.recent_route_keys = locust_core::recent_routes::recently_used_routes(&batch.summaries.values().cloned().collect::<Vec<_>>());', 's.recent_route_keys.clear();', 'recent_routes_reach_the_picker_in_history_order_and_choose_their_runtime'),
    'recent-rows': ('src/ui/thread_view.rs', 'locust_core::recent_routes::shortcut_keys(&offered, &s.recent_route_keys)', 'locust_core::recent_routes::shortcut_keys(&offered, &[])', 'recent_routes_reach_the_picker_in_history_order_and_choose_their_runtime'),
    'recent-choice': ('src/ui/thread_view.rs', 'runtime:runtime.clone(),account:Some(None)', 'runtime:None,account:Some(None)', 'recent_routes_reach_the_picker_in_history_order_and_choose_their_runtime'),
    'alias-cache': ('src/main.rs', 's.resolved_model_names = locust_core::model_alias::resolved_model_names(batch.summaries.values());', 's.resolved_model_names.clear();', 'the_picker_and_model_chip_show_the_name_earned_from_local_history'),
    'alias-row': ('src/ui/thread_view.rs', 's.resolved_model_names.get(&format!("{}:{}", agent.runtime, m.id))', 's.resolved_model_names.get("never")', 'the_picker_and_model_chip_show_the_name_earned_from_local_history'),
    'alias-chip': ('src/ui/thread_view.rs', 'let model_name = s.resolved_model_names.get(&format!("{rt}:{model}")).map(String::as_str).unwrap_or(model_name);', 'let model_name = s.resolved_model_names.get(&format!("missing:{rt}:{model}")).map(String::as_str).unwrap_or(model_name);', 'the_picker_and_model_chip_show_the_name_earned_from_local_history'),
    'helper-row': ('src/ui/activity_drawers.rs', 'locust_core::activity_entries::helper_description(name, title)', 'title', 'an_unnamed_helper_reaches_the_window_as_unnamed'),
    'handoff-row': ('src/ui/handoff_divider.rs', 'let unsettled = to_meta.checkpoint_epoch.and_then(|epoch| from_meta?.checkpoint_unsettled.get(&epoch).copied()).unwrap_or(0);', 'let unsettled = 0;', 'a_reopened_handoff_names_the_recorded_uncertain_actions'),
    'draw': ('src/ui/messages.rs', 'text(format!("Started {label}"))', 'text(format!("Began {label}"))', 'the_cached_mission_start_label_reaches_the_headless_thread'),
    'refresh': ('src/main.rs', 'pane.snapshot.started_label_minute != minute', 'pane.snapshot.started_label_minute == minute', 'an_ended_threads_start_date_refreshes_without_reopening_its_ledger'),
}
name = sys.argv[1]
file, old, new, test = controls[name]
p = pathlib.Path(file)
original = p.read_text()
assert original.count(old) == 1, ('unrestored or stale control', name)
log = pathlib.Path('notes/thread-mutation-start-' + name + '.log')
try:
    p.write_text(original.replace(old, new, 1))
    with log.open('w') as out:
        result = subprocess.run(['timeout', '590', 'python3', 'scripts/gentle-cargo.py', '--timeout', '570', 'test', '--release', '--offline', '-p', 'locust-native', test], stdout=out, stderr=subprocess.STDOUT, env=dict(os.environ, CARGO_PROFILE_DEV_OPT_LEVEL='1', CARGO_PROFILE_DEV_DEBUG='0', RUST_TEST_THREADS='4'))
    output = log.read_text()
    assert result.returncode == 101 and ('::' + test + ' ... FAILED') in output and 'test result: FAILED.' in output, (name, result.returncode, output[-3000:])
    print(name + ': named headless assertion failed')
finally:
    p.write_text(original)
    with log.open('a') as diff:
        subprocess.run(['git', 'diff'], stdout=diff, check=True)
    subprocess.run(['git', 'diff', '--check'], check=True)
