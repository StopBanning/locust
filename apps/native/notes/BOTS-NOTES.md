# Bots: glances and following the pointer (Opus, 15 Nov 2026)

Branch `exec/opus-bots`, from native-engine at `71e290a5`. This round finishes findings 2 and 3 of
`~/work/locust/docs/REVIEW-opus-native-2026-11-15.md` (brief C). The head turn from the shape (finding 1) was
already done before this round.

## What is built

- **A glance holds for 1.5 s from when it is given, then lets go.** `GlanceHold` in `core/src/bot_rig.rs` works as
  `Bot.tsx` 2393-2395 and 2476-2481 do. A glance starts its hold when it is given. While the hold lasts, it is the
  point the gaze looks at (left (-1, 0), right (1, 0), up (0, -1), down (0, 1), at full strength:
  `TeammateBot.tsx` 128-133). After 1.5 s the gaze comes back to the middle, even though the handoff that asked for
  it is still there. A different glance starts the hold again. The same glance given again does not, because React
  re-runs the effect only when `glance.x` or `glance.y` changes. A face at rest wakes for a glance and rests again
  afterwards (`wake.current?.()`).
- **Handoffs come from the workroom.** `src/ui/handoff_glances.rs`. A message is a handoff the first time the
  window's recent-list read holds it, and it lasts `RECEIVED_GLANCE_MS` (2.8 s, `faceState.ts:214`) from then
  (App.tsx 3322-3328). Messages that were already there at the first read after launch are not handoffs.
- **The roster faces are the live rig now.** Before this round, the sidebar's teammate list and the rail drew still
  pictures, so no glance could show. Electron's roster faces are live (`Sidebar.tsx` 1511-1519, and Colin's
  2026-09-15 note in `Sidebar.tsx` 1269-1278). Both now use `sidebar::roster_face`, with the roster's glances from
  `glances_among` along a column (`Sidebar.tsx:621`).
- **Following the pointer.** Once the pointer has rested on a face for `NOTICE_DWELL_MS` (220 ms), the face looks
  toward it through `pointer_pull`, measured from the face's overscanned box (`face_box`: 1.5 times the face, centred
  on it, as Electron's canvas is). It lets go when the pointer leaves (TeammateBot `follows`, Bot.tsx 2396-2398).
  This replaces the fixed, guessed `(0.25, -0.15, 0.75)` gaze.

## Tests (one per case)

- `core/src/bot_rig.rs` `glance_tests`: Electron's seven `two-teammates-look-at-each-other.test.ts` cases, plus
  `a_glance_holds_from_when_it_is_given_and_again_only_for_a_different_one`.
- `src/look/bot_stage.rs` `head_turn_tests`: one test for each side's exact pointer, `a_glance_lets_go_one_and_a_half_seconds_after_it_was_given`
  (drives the stage clock), `a_face_at_rest_wakes_for_a_glance_and_rests_again_once_it_lets_go`, and
  `a_face_that_has_noticed_the_pointer_looks_toward_it_and_lets_go_when_it_leaves` (Electron's 96-point box:
  48 points right of the head gives x 0.5 at full strength).
- `src/ui/handoff_glances.rs` `tests`: a handoff turns two faces toward each other, nothing already there at launch
  is a handoff, a handoff lasts 2.8 s, and
  `the_rosters_two_faces_turn_toward_each_other_when_one_hands_the_other_a_message` (draws the real sidebar and reads
  the two faces' rigs).

Break checks: see "Verification" below.

## Still guessed or left (and why)

- **What a roster face is doing.** Done in round 2: see "Round 2: each face's full status" below.
- **Motion level.** Roster faces use `Subtle`, TeammateBot's default (`motion = 'subtle'`, TeammateBot.tsx:361).
- **The delay before a handoff is seen.** Electron hears each posted message live. This window sees it on its next
  read of the recent list, so a glance can start later than in Electron. The 2.8 s moment counts from when the
  window heard it, as Electron's does.
- **Where a pointer is followed.** The face hears the pointer only while it is over the face (Rune's
  `on_pointer_move_local`). Electron reads it window-wide. Following only starts once the pointer is on the face and
  stops when it leaves, in both apps, so the result is the same.
- **The hover notice.** It still lasts `RECEIVED_GLANCE_MS` after it begins (the Rust stage's own rule). Electron's
  `noticed` lasts until the pointer leaves. Only the gaze was changed this round, to follow as long as the pointer
  stays. The rest of the notice (the curious mood) was left as it was.
- **Pets (Codex Buddy).** Done in round 2: see "Round 2: pets hold a glance 1.5 s" below.
- **The sidebar strip.** Electron's wide sidebar also has a row of faces (the "strip", `glancesAmong(..., 'row')`).
  The Rust sidebar has no strip, so only the column glances are wired.

## Verification

- **Full release workspace run** (`notes/bots-1-workspace.log`): exit 0, **2,089 engine + 487 window passed**,
  0 failed, 20 ignored (the real-agent cases). Every new test above was seen passing by name in that log.
- **Release build:** passed.
- **Break checks** (`~/work/scratch/mut6.py`, whole package each; `notes/bots-1-breaks.log`): seven breaks, each
  putting back the old behaviour, and **all seven were caught by the named test**:
  hold never lets go (`a_glance_lets_go_…`, `a_face_at_rest_wakes_…`); the fixed hover gaze back
  (`a_face_that_has_noticed_the_pointer_…`); a resting face not woken (`a_face_at_rest_wakes_…`,
  `the_rosters_two_faces_…`); the sidebar passing no glance (`the_rosters_two_faces_…`); messages already there at
  launch counted (`what_was_already_there_…`); the old guessed up target (`a_glance_up_…`); and the core hold
  removed (`a_glance_holds_from_when_it_is_given_…`).
- **Under the break runs, two unrelated window tests also failed once each:**
  `window_drive_tests::review_changes_shows_a_teammates_branch_turn_by_turn_and_lands_it_as_one_commit` (the known
  Git `index.lock` clash) and `window_drive_tests::relay_stops_at_hop_cap_and_exchange_strip_stop_ends_live_run`.
  Both passed in the full run on unchanged source. They are load-only failures for the next round (findings 8).
- **Snapshots** (`--snap`, scenes `rail-flyout` and `team`): the rail's faces draw as the live rig.
- `recount.py --check` passes: 191 carried, 62 does not apply, 184 partial, 872 waiting; rows 71 done, 65 partial,
  17 missing.

## Round 2: each face's full status (fixes-2, job 2)

Before this, a sidebar or rail face was only Working or Idle. Now each teammate gets Electron's one status, and
both the sidebar's roster and the rail show it.

### What was ported, and where

- **`core/src/face_status.rs`**: Electron's rules, line for line.
  - `teammate_status_view` is `status.ts` `teammateStatusView`. It gives the status (blocked, waiting on you,
    working, idle), what the face does, the words, the colour (`Tone`) and whether the name's dot pulses.
  - It includes the blocked reasons: "No AI agent installed", "Sign-in needed", "AI agent not installed",
    "This AI agent version is not supported yet", and "AI agent not answering — checking again" (amber).
  - `runtime_reach` (`runtimeReach`), `runtime_is_usable`, `state_in_words` (`stateInWords`), `face_presence_for`
    (`facePresenceFor`).
  - `live_event_of` turns a ledger event into what `live_activity_of` (faceState.ts) reads. A helper's own tool
    call (`parentItemId`) is the helper's, not the teammate's.
- **`src/ui/teammate_faces.rs`**: where the window gets each input.
  - **Agents**: what Settings › AI agents found (`settings_panel.agents`). Until that has answered, nothing is said,
    as Electron does: not known is not blocked.
  - **Running**: a live run the teammate owns, their `busy` flag, this window's own send, or the open conversation's
    running turn.
  - **Waiting on you**: the engine's cards for the teammate's runs.
  - **What a live run is doing**: read off its ledger file (`live_activity_in_ledger`).
  - **Just finished**: a run the list last saw running and now sees completed hops the face for `DONE_HOP_MS` (1.8 s).
  - **Just received**: the handoff glances from round 1.
- **On screen** (`sidebar.rs`, `rail.rs`):
  - The face shows the activity, and wears the presence dot: working white (monochrome since Electron 0.430), waiting
    amber, blocked red. The dot's colours were guessed before (a green working dot); they are now the theme's
    `text-primary`, `amber` and `red`, as `.lc-presence--*` uses.
  - The roster's name dot shows only while working (`status.pulse`), in the status's tone (white), not green.
  - The rail face tells a screen reader the hover's words as its name and the state in words as its description
    (`a-face-says-its-state-in-words`). A blocked face says why.

### Still different from Electron (and why)

- **Agents are known only once Settings has asked.** Electron asks at launch. This window asks when Settings › AI
  agents opens, so until then no face is ever blocked. That is the same as Electron in its first seconds, before
  discovery answers.
- **What a run is doing shows up to half a second late.** Electron hears each event as it streams. This window reads
  each live run's ledger file at most twice a second while anything runs.
- **"Just finished" comes from the list.** Electron hops on the `run.completed` event. Here a run hops when the
  conversation list, read again when the running turns change, shows it went from running to completed. A run that
  starts and ends between two reads does not hop.
- **Which runtime a teammate is judged by.** Electron's `runtimeOfTeammate` takes the running mission's runtime, then
  a past mission's, then the one they are set to. This window uses only the one they are set to. Those three
  Electron cases are not carried (tests.tsv).
- **The faces strip.** Electron's wide sidebar has a strip of faces. The Rust sidebar has none, so that case of
  `a-face-says-its-state-in-words` is not carried.

### Tests

- `core/src/face_status_tests.rs`: one test per Electron case, with Electron's values. Each group is headed by the
  file it comes from: `faceState.test.ts` (18 cases; its two CSS-animation cases do not apply), the teammate-status
  cases of `status.test.ts` (9), `the-bare-machine` (5), `a-teammate-is-not-stuck-…` (7), `a-signed-out-…` (6),
  `a-question-makes-the-face-wait-on-you` (2), and `stateInWords` (1).
- `src/ui/teammate_faces.rs` `tests` draw the real sidebar or rail:
  - `a_running_teammate_shows_what_its_ledger_says_on_the_roster_face_thinking_then_working`: writes a ledger, then
    a tool start.
  - `a_card_waiting_on_the_person_shows_waiting_on_the_rail_face_and_in_its_words`: a real engine card. It holds the
    engine guard, because the window counts a card's conversation as live.
  - `a_teammate_whose_own_agent_is_signed_out_shows_blocked_and_says_why_in_the_rail`.
  - `when_no_agent_on_the_machine_can_run_every_face_is_blocked_and_says_none_is_installed`.
  - `a_finished_run_hops_its_teammates_face_for_its_moment_then_rests`.
  - `a_run_already_finished_when_first_listed_does_not_hop`.

### Verification (round 2, job 2)

- **Break checks** (`~/work/scratch/mut6.py`, whole package each), 20 breaks, **all 20 caught by a named face test**:
  - 11 in `face_status.rs`, each caught by a named `face_status::tests` case (Sonnet's run,
    `~/work/scratch/opus/faces-breaks-sonnet.log`).
  - 9 in the window (`notes/faces-window-breaks.log`), each caught by a named `teammate_faces::tests` case.
- **Two things the first window break run hid:**
  - Every window break also failed `engine_calls::direct_uses_of_the_engines_turn_state_only_shrink`. That test failed
    on the unbroken tree too: the waiting-card test raises a real card on the shared approvals channel, which that
    ratchet counts. It now has a named allowance for that one test, as the Board's review test has.
  - With that noise gone, the break that drops the machine-wide answer (`runtime_reach`) was caught by nothing. The
    test `when_no_agent_on_the_machine_can_run_…` was added, and catches it.
- **Full release workspace run** (`notes/faces-suite.log`): exit 0, **2,143 engine + 499 window passed**, 0 failed,
  20 ignored. Every test named above passed by name. **Release build:** passed.

## Round 2: pets hold a glance 1.5 s (fixes-2, job 3)

- **Electron.** A pet keeps the glance and when it was given (`PetSprite.tsx` 117-120; `ScreenPet` 738-742), and looks
  that way only while less than `GLANCE_HOLD_MS` (1.5 s) has passed (`PetSprite.tsx` 183-187; `ScreenPet` 779-784).
  This is the same rule as a bot's.
- **Rust.** The pet's slot in `src/look/bot_stage.rs` keeps the same `GlanceHold` a bot's does. Its eyes and its turn
  read the held side (`GlanceHold::side_now`, `core/src/bot_rig.rs`), not the glance asked for. Before, a pet looked
  for as long as the handoff lasted (2.8 s).
- **Waking.** No extra code is needed. A new glance is part of what the pet is asked to show, so the pet is drawn again
  and wakes. Its conductor is not settled while it is turned, as in Electron's
  `codex-buddys-face-changes-as-a-screen-does` test. Two guards first written for this did nothing, and both
  break checks on them **survived**. So they were taken out rather than kept as dead code.
- **Tests** (`bot_stage::pet_glance_tests`; Electron has no test of its own for the hold):
  - `a_pets_glance_holds_one_and_a_half_seconds_then_his_eyes_come_back`
  - `a_different_glance_starts_a_pets_hold_again`
  - `a_pet_at_rest_wakes_for_a_glance_and_rests_again_once_it_lets_go`
- **Break checks** (`notes/pets-breaks.log`), all **caught by name**:
  - the pet reading the asked glance instead of the held one (all three tests);
  - the hold never started (all three);
  - `side_now` ignoring the time (`glance_tests::a_glance_holds_from_when_it_is_given_…`).
- **Full release workspace run** (`notes/pets-suite.log`): exit 0, **2,143 engine + 502 window passed**, 0 failed;
  the three pet tests passed by name. **Release build:** passed.

## Round 2: the bot, face and pet test ledger (fixes-2, job 4)

Each Electron file on `grep -nE "bot|face|pet" parity/tests.tsv | grep waiting` was read case by case against the
Rust tests (by name, and by the file's name in a Rust comment). A row is `carried` only where every case has a Rust
test of its own, and `partial` names what is missing.
- **Carried:** `a-bots-dot-and-ring-sit-on-the-bot`, `a-locust-bot-draws-before-its-first-frame`,
  `every-bot-is-drawn-by-the-rig`, `locust-bots-keep-their-eyes-on-their-heads`, and
  `a-teammate-is-not-stuck-while-agents-are-still-being-found` (job 2).
- **Partial:**
  - `codex-buddys-face-changes-as-a-screen-does`: 11 of 14. Its three drawn-screen cases have no test. A comment in
    `buddy_moves.rs` said they were in `src/look/bots.rs`; they are not, and the comment now says so.
  - `every-teammate-is-a-bot`: 9 of 14.
  - The status files from job 2.
- **Left waiting:** the other 21 files. No Rust test carries any of their cases. One name match
  (`takes_it_re_compressed…`) was checked and belongs to `codex-buddy-wears-a-screen`, not to
  `the-kept-pets-are-puppets`.
- **Hydrogen's uncommitted test work, copied:**
  - its tighter `bot_rig.rs` tests (Electron's values for the frame clock, the pointer box and the library's
    outlines);
  - `core/src/every_teammate_is_a_bot_tests.rs`. Its header said three markup cases were tested in `faces.rs`; they
    are not, and it now says so.
- **The colour test is now pinned to literals.** It compared our `shade_css` with itself. It now compares with
  literals the library gives: bot-avatars 0.2.2's `shade`, run in node: `hsl(219.7 89.7% 64.7%)` and
  `hsl(255.0 69.4% 96.5%)`.
- **Totals.** `recount.py --check` passes: 196 carried, 62 does not apply, 192 partial, 859 waiting.
- **Release build:** passed, but only after a forced rebuild. The first `build --release` after the ledger commit
  failed with `no method named side_now found for GlanceHold`, though the source and the job 3 commit both have it. The
  shared target folder (`~/work/scratch/native-target`, used by every worktree) served a stale `locust-core` to the
  window crate. Touching `core/src/lib.rs` made cargo rebuild core, and the build then passed. The same thing happened
  once in job 2. **If a build says a core item is missing that the source has, touch `core/src/lib.rs` and build
  again before believing it.** The test builds were not affected: the new tests that use `side_now` ran and passed.

