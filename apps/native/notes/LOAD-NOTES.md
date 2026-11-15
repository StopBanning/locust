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

## Not made reliable: `relay_stops_at_hop_cap_and_exchange_strip_stop_ends_live_run`

- **Seen:** it failed in 1 of 5 default-thread runs here, and once during a break run of mine (also at the default
  thread count). It passed all 5 serial runs. Sol's `notes/renderer-control-long-root.log` (09:17 today, before any
  of this round's changes) has **the same failure with the same message**, so it is older than this round.
- **What fails:** the second half's wait, "Pip's relayed run is live and ExchangeStrip sees live run", ran out its
  full **60 s** with the window at "Ready". In the fixture (`m1-app-server.py`, `relay-stop`), Pip's run, once
  started, stays live until it is stopped. So this is not slowness: either the relay to Pip never started, or the
  window never saw it live. More time would not help, so no number was raised.
- **Likely cause (not proven):** it fails only with other tests running at once. The engine keeps process-wide
  state that window tests share (`engine_api().running()`, the relay's live state, the hook guard). The first half
  of this same test also leaves a relay behind before the second half starts. Finding which takes a run with that
  state printed when the wait gives up. That is the next step for this test.

## Break check

`end_terminal_tree` with the SIGKILL to each descendant removed: **survived** with the old fixture, and **caught**
by `end_terminal_tree_kills_descendants_and_reaps_root` with `set -m` (via `~/work/scratch/mut6.py`, whole engine
suite). Source restored afterwards.
