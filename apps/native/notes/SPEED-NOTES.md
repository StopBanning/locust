# Speed 1: the Rust Locust at rest, measured (26 Nov 2026)

Colin, 26 Nov: the Rust port should be more responsive and lighter than Electron, since that was the point of it. At
rest it was not. Every number here comes from `scripts/measure-idle.sh` (row 0), three samples of 30 s after a 20 s
settle, with the spread given and the build load (rustc processes running) beside it. Raw rows:
`~/work/scratch/evidence/speed-1/idle.tsv`.

## Row 0: the measure

`scripts/measure-idle.sh` starts a SUBJECT copy of a given binary over a subject data folder, samples the window and
its engine, and appends one row per process per sample:

- **cpu %**: CPU time used during the 30 s window divided by wall time, from `ps` TIME (exact to 10 ms), not top's
  instant figure.
- **csw/s**: context switches per second (top's CSW), every wakeup of every thread. This is the wakeup figure that
  separates the two apps; top's IDLEW (idle wakeups, what the handoff quoted) barely moves for either at rest.
- **idlew/s**, **threads**, **rss MB**, **mem MB** (top's MEM: the footprint, compressed pages included; `ps` RSS
  undercounts Electron, whose pages are mostly compressed), **fds**.

How it runs a subject, and why:

- **Data**: a copy of the dogfood lane's `subject-data` (`measure-idle.sh copy`, checkpoints left out), at
  `~/work/scratch/speed-1/subject-data`. The original is served by that lane's `--serve-door` process (pid 57254):
  a second engine there would open its own door over the same `locust-mcp-connection.json` and take it over.
- **Started from the shell, not LaunchServices**: an app `open` starts asks macOS for access to the external drive
  (`~/work/scratch` is on /Volumes/Files) and waits in `open()` behind a privacy prompt; `open --stdout` is refused
  outright (-10810).
- **Focus**: Rune opens a window active, so `launch` and `front` are refused while the Mac is in use (HID idle under
  300 s). `back` gives focus back to the app that had it before the launch. Background means visible but behind
  another app's window.
- **Scenes** are reached by pressing the window's own accessibility buttons from `osascript` (Settings, Board, a
  sidebar row), which needs no focus; synthesized keystrokes (Ctrl/Cmd+3) do not reach a Rune window. A covered
  window builds no view until it is uncovered, so a press made in the background shows once it comes forward.
- **Electron** and Colin's own running Rust copy are measured read-only (`electron`, `pids`): nothing clicked.

Scenes: start (a teammate's empty conversation, where the subject opens), Settings, Board, and the longest
conversation the sidebar lists (Anywhere 1 lane, 3.7 MB ledger; the 4.0 MB Voice 2 lane is past the list's end and
not reachable without scrolling).

## Before (HEAD 2339aa12, binary d6b43d3059e4)

Front means the subject window has focus; back means visible behind another app's window.

| scene | n | cpu % (window+engine) | csw/s | idlew/s | mem MB | rustc |
|---|---|---|---|---|---|---|
| base start-front | 3 | 2.15–2.32 | 569–913 | 0–3.9 | 546–607 | 4–4 |
| base settings-front | 3 | 1.33–1.8 | 616–842 | 0.5–10.3 | 548–548 | 3–3 |
| base settings-back | 3 | 0.74–1.45 | 203–531 | 0.5–17.1 | 579–579 | 3–3 |
| base board-front | 3 | 1.33–1.61 | 590–672 | 5.5–7.9 | 631–631 | 3–3 |
| base board-back | 3 | 0.88–1.35 | 198–412 | 4–5.8 | 600–600 | 3–3 |
| base conv-front | 3 | 1.94–6.98 | 969–3488 | 2.5–26.2 | 596–596 | 3–3 |
| base conv-back | 3 | 1.98–3.85 | 210–2306 | 9.8–29.7 | 599–599 | 3–3 |

Electron (Colin's, on Team, visible in the background) and Colin's own Rust copy (live-hour build, on its own page,
visible in the background), read-only at the same time:

| scene | n | cpu % (window+engine) | csw/s | idlew/s | mem MB | rustc |
|---|---|---|---|---|---|---|
| electron team-back(visible) | 3 | 0.28–0.43 | 56–70 | 0–0.1 | 761–761 | 5–5 |
| colin-rust live-hour build, back(visible) | 3 | 9.65–10.92 | 1721–2019 | 2.1–8.9 | 382–425 | 5–5 |

What the profiles showed (window `sample` at rest, ~/work/scratch/evidence/speed-1/*.txt): the window's `poll` runs
about ten times a second because `browser_panel::poll` asked for a wake every 100 ms whenever the window existed, even
with the browser closed. Each poll then asked the engine `running`, `waiting`, `owns`, `running_checks` and
`room-notices` several times, each over a new socket for which the engine started a thread; looked up the browser's
folder (`folder_for`: a `Store` built and canonicalized, `workspace.json` opened and read); and checked the ledger
folder for `compare_view` (`Store::ledger_dir`, a canonicalize). In Colin's copy no frame was drawn in 4 s: the cost
is the poll, at 10 Hz.

## Row 1: one kept connection for reads (7f100e7d)

`engine_process::Client` keeps the connection a read was answered on (a pool of up to four, taken out while in use so a
long call never holds up another caller, closed after 60 s unused; the engine keeps a quiet connection 300 s, so the
client always closes first). Reads are `ping`, `running`, `running_checks`, `waiting`, `owns`, `room-notices`. A read
whose kept connection the engine closed (it stopped, or was started again) is asked once more on a new connection,
which changes nothing. Every request that changes something (send, stop, decide, answer, save-rule-and-decide, carry,
door, shut-down) still goes on a connection of its own and is never retried. The server answers a connection's
requests in order until it closes; a stopped `Server` shuts down the connections it was answering, so no detached
thread goes on answering for a dead engine.

Tests (core/src/engine_process_kept_tests.rs): `a_resting_window_s_reads_share_one_connection` (500 reads, 1 accept;
a stop still opens its own), `a_kept_connection_the_engine_closed_is_made_again_once` (engine replaced under a kept
connection: the next read reaches the new one, then is kept; no engine: reads empty, a change says "not running"),
`a_change_whose_reply_is_lost_is_never_sent_again` (a stop whose reply is lost reached the engine once).
Breaks: ~/work/scratch/evidence/speed-1/row1.entries, 8/8 KILLED (row1.out). No new public function.

## Row 2: a closed browser costs nothing (45240c7a)

`browser_panel::poll` asked for a wake every 100 ms whenever the window existed, so the whole window polled at 10 Hz;
each poll built a `Store` and read `workspace.json` for the browser's folder, and read `teammates.json` once a second.
Now `keep_up` returns at once, reading nothing, while the browser is closed with no tabs and no sign-ins list, and the
panel looks again in 1 s (a tab an agent opens shows within a second). Open, the folder is looked up at once when the
selected teammate or its folder changes and otherwise with the sign-ins list once a second, not every poll.

Tests (src/ui/browser_rest_tests.rs): `a_closed_browser_reads_nothing_at_rest` (100 polls, 0 reads, through the
`READS` seam), `an_open_browser_reads_once_a_second_and_sees_a_changed_folder_at_once`,
`a_resting_window_s_poll_looks_at_the_browser_again_only_once_a_second` (the poll itself, through a test-only
"attached" switch). Breaks: row2.entries, results in row2.out. No new public function.

## Row 3: no frames at rest (9e0ada3b)

A headless sweep of every scene (`--scene X --frames 600 --trace`, whose `quality.why` names what asked for each
frame) found that only Home never rests: `home`, `home-light`, `rail-flyout` and `away` all draw every frame for a
`loop` (`home-cover-beam-light`, the cover's beam while it is not ready, and the floating bot's slide). Every other
scene stops asking for frames once it settles.

Rune ran a loop whenever its element was shown, on screen, visible and motion was wanted, so Home's beam drew at the
display's rate in a window sitting behind other apps' windows. Now (vendor/rune `runtime/paint/build.rs`) a loop runs
only in a window that has focus too; behind another app's window it draws where it stands and asks for no frames. When
focus comes back, `window_focus` (`runtime/frame.rs`) marks the window dirty if it has loops, so the frame that starts
them again is drawn without waiting for another event. A window never told its focus (headless runs, snapshots) runs
loops as before.

Test (vendor/rune/crates/rune/tests/loop_focus.rs):
`loops_stand_still_in_a_window_without_focus_and_start_again_with_it` (a rotation loop: turning while focused, 0
frames asked for over 600 frames behind another app's window, a frame asked for on refocus before any other event,
turning again after it). Breaks: row3.entries, 3/3 KILLED (row3.out, after the refocus break first SURVIVED and the
test gained its refocus check). No new public function.

Not changed here: the Home cover's beam runs while Home is not "ready", which in a focused window still means frames
at the display's rate on Home. That is the cover's design (Big Brother's lead); whether it should stop once shown is
the Home owner's call.

## Row 1b: reads never wait on the engine (option A, fc7f60aa)

Colin saw the orb animate and then freeze. Each window poll asked the engine `running`, `waiting`, `owns`,
`running_checks` and `room-notices`, several of them more than once, each a round trip on the UI thread; when the
engine was busy (a turn starting, a ledger write) the window's frame waited for it. Now `engine_process::Client` keeps
the engine's last answers and a reader thread (`locust-engine-reads`) asks for all of them in one request (`reads`,
a new engine op; an older engine that does not know it is asked op by op) every 500 ms. A read returns the kept answer
at once. A send, stop, decide, answer, save-rule-and-decide, carry or shut-down drops the kept answers, so the next
read asks the engine. When the reader sees an answer change, or a room notice arrives, it wakes the window once
(`engine_api::on_change`, set by `main.rs` `setup`); while nothing changes it never wakes it. An answer the reader asked
for before this client changed something is dropped when it arrives (a change counter), so it never stands in for the
fresh one. Answers are told apart by content, not order, so the same turns listed in another order are no change.
The reader holds the client weakly and stops when the client is dropped, or once the window asks the engine to shut
down; from then on nothing starts the engine again either (an engine started by a closing window's reader would
outlive it). At rest the reader costs the engine two requests a second on one kept connection, and the window nothing.

New public functions:
- `locust_core::engine_api::on_change(Wake)`: called from `src/main.rs` `setup`, right after
  `s.changes_job.set_waker(waker.clone())` (the two lines added to main.rs: a comment and the call).
- `locust_core::engine_api::changed()`: called from `engine_process::Inner::refresh` (the `None =>` arm, when the
  client was given no wake of its own, which is every real client).

Reads are now up to 500 ms behind for what the engine changes on its own or another client asks (a turn ending, a card
raised, a turn the door or `--ask` started); what this window asks is never behind. Three reads guard something hard to
undo and would want a fresh answer: `sidebar::delete_conversation` (refuses while a turn runs),
`backup_section::confirm_restore` and `quit_guard_dialog`. A turn started by another process in the last half second
would not be counted there. A fresh read needs a new `EngineApi` method, outside this lane's files.

The turn orb is not a Rune loop: `main.rs` `poll` (2745-2756) wakes every 33 ms and rebuilds the view while the live
line shows and the window has focus, so it already stood still in a window without focus before row 3.

Tests (core/src/engine_process_kept_tests.rs): `a_read_never_waits_on_the_socket` (an engine that takes 300 ms per
answer: a read returns in under 50 ms), `an_action_makes_the_next_read_ask_the_engine`,
`a_changed_answer_wakes_the_window_once_and_an_unchanged_one_never`,
`a_room_notice_is_handed_over_once_and_wakes_the_window`, `an_answer_asked_before_an_action_is_never_kept_after_it` (a
reader's answer taken before a stop and arriving after it is dropped, not kept),
`a_closing_window_s_client_stops_asking_and_never_starts_the_engine_again`, `a_change_reaches_the_wake_the_window_gave`
(no wake of its own: `on_change`'s), `the_same_turns_in_another_order_never_wake_the_window`,
`an_engine_from_before_reads_is_asked_read_by_read`, `the_engine_answers_every_read_in_one_request`; and in
src/ui/tests.rs `the_window_wakes_when_the_shared_engine_s_answers_change` (the call site). Breaks: 6/6 KILLED: `~/work/scratch/evidence/speed-1/opta-core-1.out` (1), `~/work/scratch/evidence/speed-1/opta-core-rest6.out` (4), `~/work/scratch/evidence/speed-1/opta-native.out` (1, the `on_change` call site in main.rs).

### What each read cost before, measured (for Fix A)

`~/work/scratch/speed-1/optime.py` asked a subject engine (no window) each read the poll makes, every 100 ms for
60-75 s, the base way (a new connection per request) and the row 1 way (one kept connection), at rest and while a
Scout turn ran (OpenCode free route, count to 400). Milliseconds; rows in `~/work/scratch/evidence/speed-1/optime.tsv`;
3-5 rustc processes running throughout.

| engine | asked over | when | median | p95 | worst (op) |
|---|---|---|---|---|---|
| base 2339aa12 | new connection | rest | 0.03-0.14 | 0.08-0.25 | 548.2 (`running`), 82.1 (`owns`) |
| base 2339aa12 | new connection | during a turn | 0.06-0.16 | 0.14-0.38 | 912.7 (`running`), 101.3 (`waiting`) |
| row 2 45240c7a | new connection | rest | 0.04-0.14 | 0.08-0.25 | 14.4 (`running`) |
| row 2 45240c7a | new connection | during a turn | 0.07-0.17 | 0.21-0.65 | 8.9 (`running`) |
| row 2 45240c7a | kept connection | rest | 0.01-0.02 | 0.02-0.04 | 0.14 |
| row 2 45240c7a | kept connection | during a turn | 0.02-0.03 | 0.04-0.10 | 4.1 (`waiting`) |
| option A | kept answers | any | no socket: a read is a lock and a clone | | |

The median was never the problem; the worst was. A few base reads in each few thousand waited half a second to most of a
second for the engine to accept the connection, and the window asked on the UI thread, many times per frame, so a
frame that hit one froze the orb for that long. With option A the only reads that still ask the engine on the caller's
thread are the first one and the first after this window changed something (send, stop, decide, answer,
save-rule-and-decide, carry, shut-down): one round trip on a kept connection, worst 4 ms during a turn above.

Who asked, per frame or poll (the callers Fix A can now leave as they are, or read once and pass down):

- **Every view build** (each frame while anything animates, e.g. the orb during a turn): `teammate_faces::views`
  (`waiting`) from `rail.rs:231`, `sidebar.rs:1590` and `home_panel.rs:1487`; `sidebar::shimmering` (`running`,
  `sidebar.rs:847`); `settings_panel::updates_section` (`running`, on Settings).
- **Every poll** (`main.rs` `poll`): `os_hosts::keep_awake` (`running`, main.rs:2309), `attention_watch::poll`
  (`waiting`, `running`), `sync_approvals` (`approval_card.rs:11`, `waiting`), `home_panel::sync_board_asks`
  (`waiting`), the live list (`running` + `waiting`, main.rs:2350-2351), `room_screen::poll` (`room_notices`),
  and `ledger_tick` (`running_checks`, main.rs:773; `owns`, main.rs:606 and 802).
- **On demand** (a press, a menu): `sidebar::org_menu`, `delete_conversation`, `backup_section`, `quit_guard_dialog`,
  `missions_screen::refresh_running`, `compare_view::ensure_reruns`, `own_copies::name_of`.

Still blocking after option A: when the engine is gone, the first read after the reader finds it gone connects on the
caller's thread and may start the engine again, waiting up to 5 s (`start_engine`); that was so before too.

## Row 4: measured again (26-27 Nov)

Both builds over the same subject data, `measure-idle.sh`, three samples of 30 s each, rustc processes beside them.
That data held a face looping until at least 03:47 on 27 Nov (a conversation left mid-turn at 19:32 on 26 Nov); by
04:59 it had stopped, which is why the focused figures below come in two states.

**Behind another app's window** (the at-rest case the brief set: under 1 % CPU, wakeups at or below Electron's):

| scene | base2 2339aa12 (26 Nov 21:1x) | final 67099e62 (27 Nov 03:1x-03:3x) | Electron (Colin's, Team) |
|---|---|---|---|
| start | 25.1-26.1 % CPU, 2603-4947 csw/s | 0.12-1.05 %, 56-200 csw/s | 0.28-0.43 %, 56-70 csw/s |
| Home | 24.5-26.6 %, 2928-3858 | 0.51-0.64 %, 102-146 | |
| Settings | (base 0.74-1.45 %, 203-531) | 0.45-0.73 %, 90-115 | |
| Board | (base 0.88-1.35 %, 198-412) | 0.06-1.19 %, 55-114 | |
| longest conversation | (base 1.98-3.85 %, 210-2306) | 1.22-1.70 %, 137-194 | |
| memory (top MEM) | 684-735 MB | 521-624 MB | 761 MB |

Under 1 % in every scene but the longest conversation (1.2-1.7 %); wakeups 55-200 per second against Electron's
56-70, so at or near Electron's but not below it in every sample. rustc 1-3 throughout.

**Focused, while a face loops** (every scene the same, since the cost is the face's step, not the scene):

| build | start | Home | Settings | Board | conversation |
|---|---|---|---|---|---|
| base2 | 69.8-71.4 % | 70.3-72.1 % | | | |
| final | 61.1-64.0 % | 60.4-61.0 % | 59.9-62.2 % | 60.5-61.8 % | 58.4-62.9 % |
| front 1371591d (front-why, 1 run each, minutes after final's) | 33.5 % (final 58.5) | 35.6 % (final 59.5) | | | |

**Focused, nothing looping** (27 Nov 04:59-05:09, front-why runs 2 and 3, and an empty data folder): final 0.4-0.5 %
start, 2.4 % Home; front 0.1-0.3 % start, 0.1 % Home; one later Home run each had something moving again (final 29.0 %,
front 14.2 %, two minutes apart). Empty data folder, three runs each: final 0.4-1.5 %, front 0.9-1.2 %. Electron in
front (bench-pair, its own seeded profile, faces moving): 2.2-4.3 %.

So at rest the Rust app now sits below Electron behind other windows and level with it in front; while a face moves in
a focused window it costs about a third of a core (Left #0 under row 5). Same-machine pairs: `bench-pair.mjs`, in
BENCH-PAIR-NOTES.md (27 Nov).

## Row 5: responsiveness

### Measured before

| what | base 2339aa12 | notes |
|---|---|---|
| first frame, no data (`--bench`, team scene) | 130-138 ms warm, 1001 ms cold | r2 106-116 warm; r3 142-176 warm |
| first frame, the real app over the subject data | 1.7-2.9 s | `--bench --bench-conversation`: setup and the first poll come before it |
| longest conversation opened (347 items, 4.0 MB ledger), launch to shown | 4.25-4.77 s | r3 4.4-11.2 s; r2 11-27 s (an engine left by the turn before was still exiting) |
| a read the poll makes, worst, during a turn | 912.7 ms | optime.py, above; option A takes these off the UI thread |
| frames while focused on the start scene (every frame reported, 20 s) | r2: 294 over 16.7 ms, median 45.9 ms | slow in `poll`, hot in `people::owner_of`, `store::resolved`, `Store::seeded_json` |
| sidebar press to page drawn, key to glyph, scrolling | not measured | synthesized keys do not reach a Rune window and an accessibility press gives no drawn time; see below |

All under 3-5 rustc processes. The bench window is inactive, click-through and below every other window.

### Fixed

1. **Reads that froze the window** (option A, above): a poll asked the engine on the UI thread, and one read in a few
   thousand waited up to 913 ms during a turn.
2. **Every row's owner read from disk, every frame** (67099e62). `people::owner_of` built the path, read and
   parsed the native `teammates.json` (27 KB) and, when it did not name the conversation, Electron's too, on every
   call. The sidebar asks it for each row it draws (`row_owner`, `run_owner`), and the window draws every frame while
   the orb moves (33 ms) or a row shimmers (50 ms). In the subject data 34 of 90 conversations have no native owner,
   so each frame read and parsed two files per such row, several times over. Now `owner_of` reads each file through
   `kept_json`: parsed once and kept while its length and modified time are unchanged (a save or a write changes the
   time), so a frame costs two `stat`s, not two reads and parses per row. Tests (core/src/people_owner_kept_tests.rs):
   `a_row_s_owner_asked_every_frame_is_read_from_disk_once` (600 asks, 2 reads), `a_changed_teammates_file_is_seen_at_once`
   (a same-length change, and Electron's file written in place), `a_native_teammates_file_of_another_version_names_no_owner`.
   Breaks: 5/5 KILLED, `~/work/scratch/evidence/speed-1/row5.out`. No new public function (`kept_json` and `native_team_kept` are private;
   `owner_of`'s callers are unchanged). One behaviour gone: `owner_of` no longer seeds the native file from Electron's
   when it is missing (it read Electron's next anyway, so the answer is the same; `team` still seeds it for everything
   that writes).
3. **A focused window drew at 57-63% CPU while one face moved** (1371591d). The final build's every-frame RUNE_PERF
   spans on the start scene (891 frames, all `commit_why=poll`, median work ~26 ms) were hot in
   `rune::shape::triangulate` (1098 samples, ~55%: every bot outline's ears clipped again on each paint) and
   `store::resolved` (346, ~17%: `native_team_kept` ran `Store::check`, one filesystem call per path part, for every
   sidebar row every frame, row 5's own leftover). Now Rune keeps an outline's triangles (`shape::with_triangles`,
   crate-private: two generations of 512, matched by the points bit for bit, never by hash alone; call site
   `runtime/paint/shape.rs` `push_shape`), and `kept_json_if` checks the native file's path only when it reads the
   file again, so a symlinked or misplaced file is still refused. Tests: rune `tests/shape_kept.rs`
   `an_outline_painted_again_is_not_triangulated_again` (thread CPU of 20 repaints under 4x the first),
   `a_new_outline_with_as_many_points_gets_its_own_triangles` (a square then a dart: area 3500); core
   `a_row_s_owner_asked_every_frame_checks_the_native_file_s_path_once` (1 check for 200 asks, 2 after a change),
   `a_native_teammates_file_that_leads_into_codex_s_folder_names_no_owner`. Breaks 5/5 KILLED: `~/work/scratch/evidence/speed-1/front-rune.out` (2,
   one on the call site), `~/work/scratch/evidence/speed-1/front-core.out` (3). No new public function. Measured after (row 4): focused while a face loops 58.5 -> 33.5 % (start), 59.5 -> 35.6 %
   (Home), and 29.0 -> 14.2 % on a later Home run, each pair minutes apart on the same data.

## Big Brother change 1: a running turn moves behind another window (c3451d29)

Approved by Colin (orchestrator, 26 Nov). During a turn the orb (33 ms), the live word's sweep and a working row's
shimmer (50 ms) were held whenever the window lacked focus, so a turn beside an editor read as hung. Now they run while
any of the window shows, focused or not, and hold only when it is hidden, minimised, wholly covered (or its displays
sleep) or motion is reduced. Rune gains `Tx::shown()` (new public function; call site `main.rs` `turn_motion_drawn`,
which gates both wakes); `Runtime::transaction()` fills it from `live_visible()`. Tests: rune `tests/tx_shown.rs`
`an_update_hears_whether_the_window_shows`; native `ui/tests.rs`
`a_running_turn_moves_behind_another_window_and_holds_only_when_the_window_is_away`. Breaks 6/6 KILLED: `~/work/scratch/evidence/speed-1/bb1.out`
(4, one on the call site), `~/work/scratch/evidence/speed-1/bb1-rune.out` (2). Cost during a turn behind another window: bench-pair rows `bb1-*`.

### Left, worst first

0. **One moving face rebuilds and re-measures the whole window** (focused, 33.5-35.6 % after 1371591d while a face loops;
   0.1-0.3 % when nothing moves, row 4). Faces loop forever by design, as Electron's CSS does (`face_motion`: Waiting is
   `lcStare 6s infinite`, Thinking, Working and Responding loop too), and `bot_stage.poll` steps a looping face every
   33 ms while the window is focused. Each step changes that face's snapshot, so the poll reports a change and Rune
   builds the whole view again and measures it: in the fixed build's 10 s sample the main thread was busy ~1610 of
   4719 samples, ~790 in `view` (~300 of them `sidebar::roster_face`, every sidebar row's face drawn again from its
   SVG: `bot_face_with_look`, `draw_bot_shape`, `bot_spec::parse_svg_subpaths`) and ~990 in the measure pass. Memos
   of the drawn face and of the parsed SVG would trim the high 30s to the high 20s at best; the real fix is a face that
   steps without a whole-tree build (a retained subtree or a layer Rune repaints alone), which is Rune's and the app
   owner's design, not this lane's. Electron's ~0.3% focused was taken on Colin's data, where no face may have been
   looping: focused over an empty data folder (no face looping) both builds sit at 0.4-1.5 %, and Electron in front
   with its faces moving measured 2.2-4.3 % in bench-pair.
   Also counted there: the display link's threads, 39 (final) and 72 (front) started in 10 s, a start/stop churn
   that adds wakeups.

1. **Opening a long conversation: 4.3-4.8 s** (base, launch to shown, 347 items, 4.0 MB ledger; 1.7-2.9 s of it is
   before the first frame). Not paired with Electron yet. Profile: `profile-open.sh` (a `sample` of
   the bench window from launch), results under "After".
2. **The real app's first frame: 1.7-2.9 s** against 106-176 ms for the same binary with no data, so the cost is the
   app's setup and first poll over the data, before anything is drawn.
3. **The sidebar is worked out again on every poll.** `sidebar::shimmering` builds the whole drawn list
   (`drawn_missions`: every row filtered, ordered and cloned) to learn whether one row shimmers, every poll and every
   50 ms while one does, and `sidebar_missions_now` runs again for the view. Keep the drawn list until the recents,
   the org file or the search change.
4. **Home, focused, draws every frame** (the cover beam's loop while Home is not ready, and the floating bot's slide):
   not measured cleanly (the evening's focused runs were taken while a video played and read ~72% for every scene,
   start included, so they are set aside). A design call for Home's owner (row 3).
5. **Not measured: sidebar press to page drawn, key to glyph, scrolling.** Synthesized keystrokes never reach a Rune
   window and an accessibility press carries no drawn time. Measuring these needs a hook in the app (a press or key
   stamped at input and at present, as `--bench` stamps its first frame), which is app code outside this lane.

## For other owners (not changed here)

- **compare_view.rs** (`poll`): `store.ledger_dir()` (a `Store::check`: lstat + canonicalize) runs on every poll before
  it knows whether any compare is running; it was the second-largest item in Colin's window profile. Check for running
  compares first.
- **main.rs** (`poll`, Fix A): one poll asks the engine `running()` three times (keep_awake, the live list, attention)
  and `waiting()` two or three times (sync_approvals, sync_board_asks, the live list). Read each once per poll and pass
  it down. With row 1 each is one round trip on a kept connection, but each is still on the UI thread.
- **attention_watch.rs**: `notify::supported()` calls `std::env::current_exe()` on every poll.
- **native::store()** (core/src/native.rs, not in this lane's files): builds and checks a `Store` from disk on every
  call; the engine and the door call it often.

# Speed 2: the worst remaining costs (27 Nov 2026)

Branch exec/native-speed-2 (cut at 31ae451a, fixturn-1 f7b4a19b merged). Colin was at the Mac throughout, so every
figure here is headless: no subject window was opened on his screen, which rules out `measure-idle.sh` (it needs the
Mac idle and a visible window) and the `--bench` window `profile-open.sh` starts (level -1, so it shows wherever the
desktop is bare). The data is a copy of the dogfood lane's `subject-data` (`measure-idle.sh copy`, checkpoints left
out) at `~/work/scratch/speed-2/subject-data`, with `HOME` pointed at `~/work/scratch/speed-2/home`, so nothing of
Colin's or Electron's was read. The brief allowed no release build, so these are dev-profile binaries (locust-native
and locust-core at opt-level 0, dependencies at 2): read them against each other, not against Speed 1's release
figures.

## The measure: `--bench-headless`

`--bench-headless <out.json> --bench-conversation <id>` (main.rs `run_bench_headless`) runs what `--bench
--bench-conversation` runs, the app's own `setup`, `poll` and `view` over the data the environment names, opening that
conversation through `open_when_listed`, but on a `rune::Runtime` with no window, stepped on the wall clock at 60 Hz.
Each frame builds, lays out and paints the view (everything but the GPU). It writes, in ms from process start:
`setup_ms`, `first_frame_ms`, `listed_ms` (the row was listed and the open began) and `opened_ms` (the conversation
is shown with its items, the same `<out>.opened` proof the window bench writes). Run by
`~/work/scratch/speed-2/open.sh <tag> <binary> [n]` (Speed 1's `profile-open.sh` and `bench.sh` in one, headless; with
`SAMPLE=1` a `sample` of the first run). Raw results: `~/work/scratch/speed-2/bench/`.

## What a launch that opens the longest conversation costs (before: f7b4a19b with only the bench flag added, `bins/before`)

Voice 2 lane (`mission_native_44397_1795526334661268000`, 347 items, 4.0 MB ledger), three runs, 1 rustc running:

| run | setup | first frame | listed | opened | open after listed |
|---|---|---|---|---|---|
| before 1 | 1824 | 2114 | 4770 | 5173 | 403 |
| before 2 | 1164 | 1168 | 3963 | 4300 | 336 |
| before 3 | 1175 | 1178 | 4072 | 4439 | 367 |

The `sample` of the main thread and the workers (`bench/beforeprof-sample.txt`) says where the time goes:

1. **Setup reads every ledger before the first frame** (row 3). 1267 of setup's 1296 main-thread samples are
   `composer_wiring::load_composer_store` -> `limits::limited_runtimes` -> `usage_ledger::collect_missions`: every
   `.jsonl` in the ledger folder (87 files, 112 MB) read and parsed into `serde_json::Value`s to learn which runtimes
   are at their usage limit. The first frame follows setup within milliseconds, so this is the whole first-frame cost.
2. **The list waits on a cold read of every ledger, done twice at once** (row 2). Once listed, the conversation itself
   opens in 336-403 ms: the 4.0 MB ledger is not the cost. Until then `open_when_listed` waits for the row to be listed,
   and listing (`refresh_recent` -> `native::list_recent` -> `HistoryCache::read_history_at` -> `ledger::read_ledger`)
   parses every ledger cold. setup's `load_team` asked for the list before the engine was known; when the team landed a
   moment later, poll asked again (main.rs, "the first list was asked before the engine was known"), which dropped the
   first read's answer (`rune::Latest` keeps only the newest request's) and started a second cold read beside it. Two
   `RecentBatch` workers parsed every ledger side by side, and the list came when the second finished.
3. **A third full parse runs beside them**: `agent_card_ui::request` -> `usage_ledger::usage_windows` ->
   `collect_missions`, started by each `refresh_recent`.

So a launch parses the whole ledger folder four times (setup's limits on the main thread, two list reads, the usage
windows), none of them for the conversation being opened.

## Row 2: the launch's list read is kept (main.rs `list_read_holds`)

When the team lands and the list read asked before it is still running, it is kept, not asked again, provided its
answer is the one a new read would give: `native::list_recent` answers the native engine differently only while the
native ledger folder is not made yet, so a read over a folder that is there holds (as does any read for the door
engine). A read that has already landed is asked again, as before. This is the pattern `room_screen::refresh` and
`agent_card_ui::request` already follow (neither asks while its own read runs).

After, three runs each (binary `bins/r2`):

| run | setup | first frame | listed | opened | listed after first frame |
|---|---|---|---|---|---|
| before | 1164-1824 | 1168-2114 | 3963-4770 | 4300-5173 | 2656-2894 |
| after | 1202-1801 | 1206-1972 | 3635-4177 | 3984-4521 | 2205-2587 |

About 0.3-0.4 s off the listing, so the open: 3.98-4.52 s against 4.30-5.17 s in this debug build. **Not the 1.5 s
target.** What is left is the cold parse itself and setup's, both in files outside this lane (below).

Tests (src/ui/list_read_holds_tests.rs, through `poll`'s team arm, the call site):
`a_list_read_running_when_the_team_lands_is_kept_not_asked_again`,
`with_no_native_folder_yet_the_list_is_asked_again_for_the_native_engine`,
`for_the_door_engine_a_running_read_is_kept_with_no_native_folder_too`,
`a_read_that_already_landed_is_asked_again_when_the_team_lands`. Breaks: 4/4 KILLED, each by these tests
(`~/work/scratch/evidence/speed-2/row2.entries`, `row2.out`), one on the call site in `poll`'s team arm. Those runs also
failed a room_screen source check, because row 4's test counter then stood above sidebar.rs's code (that check reads the
file up to its first test attribute); the counter was moved below the code before either row was committed.
No new public function (`list_read_holds` is private to main.rs, called from `poll`).

## Row 3: setup's limits read runs off the window's thread (composer_wiring.rs `limits_job`, main.rs `take_limits`)

Setup read every ledger on the window's thread to learn which runtimes are at their usage limit
(`load_composer_store` -> `limits::limited_runtimes`). Colin allowed the two files outside this lane's (27 Nov):
`load_composer_store` now starts that read as a `Latest` job (`limits_job`, one field in app_state.rs) and poll sets
`limited_runtimes` when it lands (`take_limits`, main.rs). Until it lands the composer treats no runtime as limited, as
it does with no ledger; opening the model menu and the agents screen still read it again on the spot, as before.

An experiment build first (`bins/exp-nolimits`, never committed: the line taken out) showed what moving it would buy;
then the change itself (`bins/r3`, HEAD 96e98bd8 + this change, 1 rustc running beside it), three runs each:

| run | setup | first frame | listed | opened |
|---|---|---|---|---|
| before (row 2's build) | 1202-1801 | 1206-1972 | 3635-4177 | 3984-4521 |
| without setup's limits read (experiment) | 6-132 | 11-320 | 2506-3042 | 2847-3401 |
| after (row 3) | 6-195 | 10-489 | 3004-3329 | 3418-3762 |

First frame 10-489 ms (the 489 is the first run after the data was copied, cold disk) against 1.2-2.0 s: **under the
0.8 s target**. The list read starts sooner too (load_team comes after `load_composer_store` in setup), which takes the
open to 3.4-3.8 s; the rest is row 2's cold parse (below).

Test (src/ui/limits_off_launch_tests.rs, in a child process with `LOCUST_NATIVE_DATA` on a ledger holding a codex
limit, through `poll`, the call site): `setup_leaves_the_limits_read_to_a_job_and_poll_sets_what_it_found` (no limit
known when setup returns, codex limited after polls). Breaks: 3/3 KILLED (`~/work/scratch/evidence/speed-2/row3.entries`,
`row3.out`), one on the call site in poll. No new public function (`take_limits` is private to main.rs).

## Row 4: a resting window never builds the drawn list (sidebar.rs `shimmering`)

`sidebar::shimmering` is asked on every poll while the window shows, and every 50 ms while a row shimmers. It built
the whole drawn list (`drawn_missions`: every row grouped, filtered, ordered and cloned, each row's owner looked up) to
learn whether one row is running. A drawn row is running only when a listed row is (`recent_as_missions` takes a row's
phase from its leaf's state, and `conversation_rows` only sorts), so with none running the answer is now false
without building the list. With one running, the answer is still the drawn list's own, so a running row the list
hides (a face filter, a folded group, a settled row) never shimmers.

Not done as the brief put it ("keep it until recents, the org file or the search change"): the drawn list also turns
on the selection, the face filter, the source switch, folded groups, comparisons, the Board's asks and the clock (a
settled row is put away by time), and while a turn runs the view builds the same list every frame anyway (the orb
rebuilds the view at 33 ms). At rest, where the poll was paying for it alone, it is now not built at all.

Tests (src/ui/sidebar_shimmer_rest_tests.rs): `a_resting_window_s_poll_never_builds_the_drawn_list` (200 polls over
90 listed rows, none running: 0 builds, counted by `DRAWN_BUILDS`),
`a_running_row_shimmers_and_a_running_row_the_list_hides_does_not`. Breaks: 3/3 KILLED by these tests
(`~/work/scratch/evidence/speed-2/row4.entries`, `row4.out`; `row4-first.out` is the run from before the counter moved).
No new public function.

## Row 1: not started (outside this lane's files)

One moving face rebuilds and re-measures the whole view because the face is a `canvas(shape)` built in the view
(`src/look/faces.rs`, `bot_face_with_look` 306-347: `snapshot_bot` -> `draw_bot_shape` -> `canvas(shape)`), and
`bot_stage.poll` reports each step as a change. A face that moves without a rebuild needs Rune to draw a shape it is
handed between builds and the face's drawing to be redone from the poll:

- Rune: a `LiveShape` (a shared, versioned `Shape`, as `LiveText` is for text and `Pixels` for images) and
  `El::live_shape` / `canvas_live`: subscribed in `subscribe_live` (live_latch.rs), swapped into the node's
  `vis.shape` in `latch_live`, never kept in a cached layer (cache_layer.rs, as `pixels`), compared by source in
  memo_reuse.rs. The frame then paints the new outline with no commit and no measure, as a `Live` value does.
- bot_stage.rs: each slot keeps its `LiveShape` and what the view drew it with (shape, face, hue, size, theme,
  presence, ring); `poll` steps the rig, draws the new outline into the slot's `LiveShape` and reports no change for
  those faces (puppets still rebuild).
- faces.rs: hand the slot its drawing and use `canvas_live` instead of `canvas(shape)`. faces.rs is not
  `src/look/bot_*.rs`, so this lane stopped here rather than edit it.

## Left, worst first (after Speed 2)

0. **One moving face rebuilds the whole window** (row 1, above): needs `src/look/faces.rs` (about ten lines) beside
   the Rune and bot_stage.rs work.
1. (done, row 3) Setup's limits read now runs off the window's thread.
2. **A launch still parses the ledger folder three times** (row 2's rest): the list (history_cache.rs, cold at every
   launch: the cache lives in memory only), the usage windows (usage_ledger.rs, read again in full on each
   `refresh_recent` that is not already running one) and the limits job (row 3, now off the window's thread). `limited_runtimes` and
   `usage_windows` both come from `collect_missions` over the same files and could share one read; the list needs only
   summaries for all but the newest 20 rows yet parses every record of every file. Those files (history_cache.rs,
   usage_ledger.rs, limits.rs, agent_card_ui.rs) are outside this lane's.
