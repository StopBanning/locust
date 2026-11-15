#!/usr/bin/env python3
"""Check one start-label window regression headlessly, restoring before exit."""
import os
import pathlib
import subprocess
import sys
controls = {
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
        result = subprocess.run(['timeout', '5400', 'python3', 'scripts/gentle-cargo.py', '--timeout', '5380', 'test', '--release', '--offline', '-p', 'locust-native', test], stdout=out, stderr=subprocess.STDOUT, env=dict(os.environ, CARGO_PROFILE_DEV_OPT_LEVEL='1', CARGO_PROFILE_DEV_DEBUG='0', RUST_TEST_THREADS='4'))
    output = log.read_text()
    assert result.returncode == 101 and ('::' + test + ' ... FAILED') in output and 'test result: FAILED.' in output, (name, result.returncode, output[-3000:])
    print(name + ': named headless assertion failed')
finally:
    p.write_text(original)
    with log.open('a') as diff:
        subprocess.run(['git', 'diff'], stdout=diff, check=True)
    subprocess.run(['git', 'diff', '--check'], check=True)
