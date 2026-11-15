# Tests that failed only under load (Opus, 15 Nov 2026)

Finding 8 of `~/work/locust/docs/REVIEW-opus-native-2026-11-15.md` (brief D). Branch `exec/opus-bots`.

## The rule now

A test that waits for another process (a shell, a fake agent, a Python server) waits for **the thing it checks**,
polling every 10 ms, and gives up only after `load_waits::UNDER_LOAD` (60 s; `core/src/load_waits.rs`). When the
event comes, the test goes straight on, so the limit costs nothing on a quiet machine. A short limit is what failed
these tests on unchanged source while four lanes were building.

## Each test: why it was slow, and what changed

| Test | Why it failed under load | Change |
| --- | --- | --- |
| `runtime::process::tests::end_terminal_tree_kills_descendants_and_reaps_root` | Two real causes. (1) It slept 80 ms and then expected the shell's children to exist; a busy machine can schedule the shell later than that. (2) It checked the killed children the instant after the kill. A killed child whose shell is gone is an orphan, and launchd reaps it in its own time, so `ps` still listed it ("descendant … still alive", Sol's run of 15 Nov). | It waits for both children to appear, and then for each to be gone. **It also never tested what its name says:** the sleeps were in the shell's process group, so killing the group ended them even with the walk down the descendants removed (a break check of mine **survived**). Now `set -m` gives each sleep its own group, as a terminal's jobs have. The same break is then **caught** by this test by name. |
| `terminal::tests::drop_kills_and_reaps_pty_shell` | Waited at most 2 s for the shell in the pty to write its pid. | Waits for the pid, up to the load limit. The check after `drop` stays immediate: drop waits for the shell, so it must be gone when drop returns, and that is what the test checks. |
| `shell_path::tests::a_real_login_shell_answers_with_a_path` | The app gives the login shell 3 s (Electron's `execFileSync` timeout, `shell-path.ts:67`). A zsh starting while four builds run can take longer, and then the test failed on "zsh answers". | The app keeps 3 s. The function takes its budget as an argument (`login_shell_path_within`), and the test gives the real shell the load limit. The test checks that a real shell answers with a PATH, not how fast. |
| `scout::host_tests::on_diagnostic_from_opencode_provider_busy_asks_next_free_model` | Gave the whole fallback 6 s: two runs against a Python mock server. | Waits for the card, up to the load limit. |
| `runtime::muse_tests::muse_teammate_uses_native_driver`, `muse_fake_process_writes_ledger` | 8 s and 15 s for a fake Muse process. | The load limit. |
| `native::tests::relay_prose_return_posts_unanswered_prose_back_to_asker` | Already waited on the relay's own state, with 30 s (raised from 6 on 14 Nov). | The load limit. The module's `wait` helper too (20 s before). |
| `runtime::permission_bridge::tests::permission_tool_routes_decisions_and_refuses_after_end` | Already waited for its own request, with 30 s (raised from 10 on 14 Nov). | The load limit, so every such wait has one number. |
| `profile_backup` restore (`window_drive_tests`) | Its waits are the window pump's, already 60 s. | No change; it passed all ten runs. |

`load_waits::note_time` writes each changed test's time to a file when `LOCUST_LOAD_TIMES` is set. Without it,
it does nothing.

## Ten full runs on unchanged source

`~/work/scratch/opus/load-runs.sh`: full release workspace, `--no-fail-fast`, five at `RUST_TEST_THREADS=1`, then five
at the default. Logs are `notes/load-{serial,default}-N.log`, with times in `notes/load-*-N.times.log` (git ignores
`*.log`, so they stay on this machine).

| Run | Result |
| --- | --- |
| serial 1-5 | all passed: 2,089 engine + 487 window each, 0 failed (246 s, then about 128 s each) |
| default 1-4 | all passed: 2,089 + 487, 0 failed (about 84 s each) |
| default 5 | **1 failed**: `window_drive_tests::relay_stops_at_hop_cap_and_exchange_strip_stop_ends_live_run` (below) |

**So "five runs in a row with no failure" is met at one thread, and not at the default.** The one failure is not
one of the tests this round changed.

Worst time seen over the ten runs, in milliseconds (serial / default):

| Test | Serial | Default |
| --- | ---: | ---: |
| `a_real_login_shell_answers_with_a_path` | 23 | 25 |
| `drop_kills_and_reaps_pty_shell` | 86 | 85 |
| `end_terminal_tree_kills_descendants_and_reaps_root` | 58 | 51 |
| `muse_fake_process_writes_ledger` | 53 | 56 |
| `muse_teammate_uses_native_driver` | 97 | 134 |
| `on_diagnostic_from_opencode_provider_busy_asks_next_free_model` | 363 | 348 |
| `relay_prose_return_posts_unanswered_prose_back_to_asker` | 390 | 333 |

**What these numbers do not show.** The machine's load average was about 3 during these runs (`uptime`: 2.83 3.21
3.87). That is not the four-builds load of 14 Nov, when these tests failed, so the times above are a quiet
machine's. What the change gives under heavy load is room: 60 s against worst quiet times under half a second.
That was not measured under heavy load this round.

## Round 2: why the window tests failed only in parallel, and the fixes

Three window tests failed only when tests ran at once (the default thread count), never one at a time. Each had a
real cause. None was fixed by raising a number.

### 1. `relay_stops_at_hop_cap_and_exchange_strip_stop_ends_live_run`: the list was read too early

- **How it was found.** Its 60 s wait now says what it found when it gives up (`pump_saying`, and
  `native::relay_debug()` in the engine, test builds only). The next failure (`notes/relay-before-fix/relay-fix-default-1.log`)
  showed the engine side complete: Pip's relayed run was running, active, and in the window's `seen_live`; Pip's hub
  existed; the team file was right. What was missing was the window's Exchange view: the exchange was found but had
  **no live run** (`exchange=Some([])`).
- **Cause.** The engine counts a run as running (`core/src/native.rs`, the slot is claimed in `send`) a moment before
  it writes the run's owner and the "received" link to the message it answers (about 150 lines later in `send`). The
  window reads its conversation list once, when the running turns change. Under load that read landed in the gap:
  Pip's run had no owner and no link, so it was in no exchange, and nothing read the list again while it ran. **A
  person could meet this too**: the Exchange strip would show no Stop for a live relayed run.
- **Fix** (`src/ui/live_list.rs`, called from `poll`). While a live run has no owner in the list, the window reads the
  list again: once a second, and for at most ten seconds a run. The engine's order is left alone, since claiming the slot
  first is what stops two turns starting on one conversation.
- **Tests.** Five cases for the rule in `live_list::tests`, and
  `window_drive_tests::a_live_run_the_list_was_read_too_early_to_know_is_read_again_until_it_is_there`. That test
  lands a list read made in the gap, then waits for the owner to come back. **Break check:** with the re-read turned
  off, that named test failed (`mut6.py`). The first version of the test did not catch the break (the read it
  waited out was finished but not yet applied); it was rewritten to land an early read itself.

### 2. `a_relay_history_refresh_names_the_received_message_in_the_sidebar`: another test's turn replaced its list

- **Cause.** The test hands the window a list read of its own and waits for it to land. The window reads its list
  again whenever the engine's running turns change, and the engine is the whole test process's. So another test's
  turn starting beside this one made the window ask again, and the newer read replaced this one before it landed
  (failed 5 times in about 39 window-suite runs while hunting).
- **Fix.** The test holds the engine guard (`native::hooks::install`) while it waits. Every test that runs a turn
  holds that guard, so no turn can start or end beside it. This race cannot be forced, so there is no break check.
  The ten runs below are the evidence.

### 3. `review_changes_shows_a_teammates_branch_turn_by_turn_and_lands_it_as_one_commit`: git's optional lock

- **Cause.** The window reads the folder with `git status` while the landing commits. `git status` takes
  `.git/index.lock` for a moment to refresh its cached file times, and the commit found the lock taken ("Unable to
  create index.lock: File exists"). **A person could meet this too**: Locust's own reads could refuse their landing,
  or their own `git commit` in a terminal.
- **Fix** (`core/src/git_quiet.rs`). Every git Locust starts to read or write a folder (`changes::git`,
  `worktrees::run_git`, `folder_commit`, `compare_copies`, Scout's two) is given `GIT_OPTIONAL_LOCKS=0`, as editors
  that watch a repository are. Locks a write needs are not optional and are still taken. Electron does not set it;
  nothing a person sees changes. The test no longer needs the variable in its environment (Sol's runs set it).
- **Tests.** `git_quiet::tests::a_plain_git_status_rewrites_a_stale_index` is the control: plain git does rewrite the
  index. `the_changes_panels_git_leaves_the_index_alone` and `a_teammates_branch_read_leaves_the_index_alone` check
  the app's two main readers. **Break check:** with the variable taken out of `changes::git`, the named test failed.

### Also fixed: a turn's ending ran on into the next test

A turn leaves the engine's running list before its thread has done its ending: the relay's hand-off, rooms, Scout,
carry-on. The engine guard waited only for the running list. So a turn's ending could act on the next test's store
and make a relay in the next test's state. In test builds, the engine now counts turn threads, from before the spawn
until the thread ends (`TURN_THREADS`, `TurnThread`). The guard waits for none, both when a test lets go and when the
next one takes it, stopping any turn an ending starts. Test:
`native::tests::a_test_lets_go_of_the_engine_only_after_every_turn_thread_has_ended`. **Break check:** with the wait
removed, the named test failed. This was not the cause of finding 1: that test failed again after this fix, before
the list fix (`notes/relay-before-fix/`, runs 1 and 5).

### Ten full runs on the final source, default thread count

`notes/relay-final-default-1..10.log` (git ignores logs; they stay on this machine): **all ten passed, none failed**:
2,093 engine + 493 window each, 20 real-agent cases ignored. Every test named above passed by name in all ten.
The release build passed.

On the way there, a batch on a nearly final source (`notes/relay-fixed-round-a/`) passed the window suite ten times
out of ten, but failed one engine test once:
`native::tests::relay_roundtrip_starts_recipient_records_peer_links_remembers_hub_and_returns_to_origin_thread`. It
had a fixed 6 s limit, and its message showed the round trip still going (Pip's hub made, Codec's follow-up running).
It now waits up to the load limit, as round 1's tests do, and reads its locks into values before panicking. Its old
message held an engine lock while panicking, which poisoned it, and a turn thread then panicked on it.

### Short fixed limits still in the tests (not changed this round)

They are not known to fail, and each needs reading before it is changed:
`src/ui/daily_controls_tests.rs:213`, `:240` and `src/ui/tests.rs:365`, `:505`, `:557` (2 s);
`core/src/browser_tests.rs:448` (3 s); `core/src/acp_agent_tests.rs:142`, `:158`,
`core/src/github_account_tests.rs:170` and `core/src/remote_control_tests.rs:83` (5 s);
`core/src/native/tests.rs:1298` (8 s); `core/src/engine_process_tests.rs:146` (2 s).


## Break check

`end_terminal_tree` with the SIGKILL to each descendant removed: **survived** with the old fixture, and **caught**
by `end_terminal_tree_kills_descendants_and_reaps_root` with `set -m` (via `~/work/scratch/mut6.py`, whole engine
suite). Source restored afterwards.
