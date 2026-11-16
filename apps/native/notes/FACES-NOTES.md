# Faces, marks and the face test ledger (round 6, 16 Nov 2026)

Branch `exec/opus-faces` in `~/work/locust-wt/opus2`, cut from native-engine 6cd55444. Run by Claude Sonnet 5.5 from
`HANDOFF-opus-6.md` (written for Opus). Parity rows 3 and 14.

How the work was checked: every new test has a break check. A break is a one-line change to the real code that should
make exactly that test fail; the script `~/work/scratch/opus2/brkm.py` applies several independent breaks at once and
reports, for each, whether its NAMED test failed (`brk.py` does one). Where a break was caught by a different test, or not
at all, it is said below.

A lesson from this round: a break only counts if the test PASSES before it. `the_motion_a_face_asks_for_is_never_computed`
was "killed" by a break while it was already failing (it rejected `motion: BotMotionLevel,` field declarations in
`bot_stage.rs`); the full suite found it. It is fixed, run clean, and re-broken (`other failures: 0`). The intermediate
commits were each cut by hand from one tree (registrations in `lib.rs` and `look/mod.rs` added job by job); only the
last one, and the working tree, were built and run whole.

## Job 1: AgentMark (and AgentCard)

**Correction to the hand-off.** `the-runtimes-wear-their-marks.test.tsx` is about `RuntimeMark` (the drawings), not
`AgentMark`. AgentMark's own cases are in three other files: `an-account-shows-how-full-it-is`,
`antigravity-shows-remaining-not-used`, `the-model-chip-shows-usage-on-hover`. All four are in `parity/tests.tsv` now.

What Electron's AgentMark does (`components/AgentMark.tsx`): a hover/focus card with the agent's name, its state, a bar
for each usage window (amber from 80%, red at 100%, a window reset since the reading drawn as "reset since"), and where
the reading came from. It is shown in two places: Home's "Connected accounts" row of marks (`FirstLaunch.tsx`) and above
the chat box's model chip (`Composer.tsx`, `AgentCard`), where the chip also wears an amber dot from 80% and says its
usage aloud.

Built in Rust:
- `core/src/agent_card.rs`: the card as a value (`agent_card`), the mark's spoken name (`mark_said`), the chip's spoken
  usage (`chip_usage_sentence`), the amber dot rule (`chip_is_pressing`), `ready_state`, `chip_version` (drops a build
  hash). The usage text itself was already `core/src/usage_words.rs`.
- `src/ui/agent_card_ui.rs`: draws the card over the model chip on hover (above, right edge aligned, as
  `.lc-agentcard--chip`), and reads the ledger's latest readings (`usage_ledger::usage_windows`) in a background job.
  `thread_view.rs` hangs it on the chip, adds the amber dot after the chevron, and sets the chip's description.
- The readings are asked for from `refresh_recent` (so the dot and the description are there without a hover) and again
  when the pointer first rests on the chip.

Not built (said in `tests.tsv` and `rows.tsv`):
- Home's row of agent marks with their cards: Rust's Home "Connected accounts" is a placeholder
  (`home_panel::boot_view_ui`). `mark_said` has no mark to hang it on.
- The card opening on keyboard focus (only hover).
- Of `the-runtimes-wear-their-marks` (15 cases) 6 have Rust tests (`src/look/runtime_marks_tests.rs`): the rest are the
  label option, the muted Gemini, the composer chip's greying, the approval card's and picker's marks, the badge.

Break checks: all caught by the named test, except these notes. (a) `the_model_button_wears_its_amber_dot_from_80` at
exactly 80 first survived; the test now asserts 79 and 80. (b) Putting the card's side at `Bottom` is not caught:
`anchored_fit` flips it above when it does not fit, so it is the same card. The alignment (`Align::End`) is caught.

## Job 2: PixelFace

`PixelFace.tsx` is **not drawn anywhere in Electron**. `grep '<PixelFace'` over the renderer finds nothing; `TeammateBot`
(the rig) replaced it. What is still used is its prop types (`PixelFaceProps`, `FaceActivity`) and `PRESENCE_TONE`, which
`TeammateBot` imports. So there is nothing to draw. The tone is already in Rust (`look/bots.rs presence_color`); this round
adds `presence_tests::the_presence_dot_is_amber_for_approval_red_for_blocked_and_the_text_colour_for_work_in_both_themes`
(break check: Blocked made amber is caught). Row 3's detail says so.

## Job 3: a pet plays what its teammate is doing

New `core/src/pet_motion.rs` and `pet_motion_tests.rs`, registered after `pets` in `lib.rs`: `pet_state_for`, `PetCell`,
`resting_cell`, `held_failure_cell`, `pet_cell_at`, `pet_fade_ms` with `PET_FADE_MAX_MS`, `pet_next_change_in`,
`pet_glance_cell`, `painted_brightness` with `DARK_PET`. `pets.rs` is untouched; nothing was needed from it. 11 of 11
cases carried; each broken and caught. Times are `f64` ms as in Electron; `pet_fade_ms` returns whole ms (Electron's
`Math.round`). Rows are `u8` 9 or 11, as `pets::pet_rows_for` gives them.

## Job 4: the face test ledger

All the new tests are in `core/src/face_ledger_tests.rs` (named for the Electron case), plus
`src/look/face_motion_sites_tests.rs`. Result per file (see `parity/tests.tsv`):

| Electron file | Now | Left |
| --- | --- | --- |
| a-face-at-rest-answers-you | carried 7/7 | |
| a-face-rests-behind-other-windows | partial | the 5 window-presence cases (blur/focus/hidden/loading window/server render): the stage takes a `focused` flag, with no tests of those rules |
| a-screen-keeps-its-face-to-you | carried 5/5 | `screen_pose` takes the pose by value, so "never changes the rig's pose" holds by the type and cannot be broken |
| a-teammate-at-work-goes-round-its-faces | carried 12/12 | |
| a-thinking-face-looks-with-its-head | partial 7/12 | the 5 "a screen's eyes" cases read Electron's canvas strokes |
| every-face-draws-on-one-beat | carried 3/3 | |
| every-face-measures-before-any-face-writes | does-not-apply | no layout to read in Rune (reason in the ledger) |
| only-the-face-you-talk-to-hops (main) | partial 3/5 | CSS keyframes and reduced-motion media query do not apply |
| only-the-face-you-talk-to-hops (renderer) | partial 4/6 | the two markup cases |
| one-fact-one-surface | partial 2/10 | **not built**: see below |

Things found while doing this (not fixed, outside the job):
- **one-fact-one-surface:** Rust has no `missionPhaseView` ("receipt incomplete" is amber everywhere) and
  `benchmark::cost_cell` answers `—` for a missing cost where Electron says `not reported` (Yurt's beta report #14). Only
  `inspector.rs` has the words. A shared `cost_line_or_why_not` would be one small port.
- **`BotFaceOpts::default()` asks for `BotMotionLevel::Full`;** Electron's `TeammateBot` is subtle unless told. Only tests
  build from the default today (`face_motion_sites_tests` holds the list of real call sites). Changing it would move the
  `teammate_bot_markup_tests` cases, so it was left.
- Electron's "full in three places" has the live line in a conversation and the call with a teammate; the Rust window
  asks for Full only in the New teammate preview.
- Two break checks interacted: with the live-activity `if streaming && open_tools.is_empty()` removed, the
  `streaming = false` break showed as survived. Run alone it is caught
  (`a_tool_loop_whose_steps_carry_words_never_replies_until_its_answer`).
- The `on_presence` stopped-guard and `schedule`'s `!self.stopped` are two defences of one rule: removing either alone is
  not caught; removing both is (`a_face_that_has_stopped_asks_for_nothing_after_whatever_its_window_does`).

## Job 5: four leftovers from the review

1. **The "an older one" git message.** The wording was already tested (`review_changes.rs`, both `OldGit` cases). What had
   no test is `gather_landing` returning `OldGit` when git cannot say its version. A test cannot change the process's `PATH`
   (every test running beside it spawns real git), so `worktrees::Manager` has a seam: `with_git_program(path)` and
   `run_git_with(program, ..)`. The tests give it a script that is the real git except for `--version`:
   `landing_is_blocked_with_no_version_when_git_cannot_say_its_version` (the script exits 1, so the version is None and the
   message says "an older one") and `landing_is_blocked_with_the_version_when_git_is_older_than_2_38` ("2.30.1"). Both
   broken and caught. `land` is checked too (blocked, nothing touched).
2. **Recent routes, equal times.** The premise did not hold: `recently_used_routes` keeps routes in a `Vec` in first-seen
   order and uses its `HashMap` only to find a route's place; `sort_by` is stable. That is what Electron's `Map` plus `sort`
   does. `routes_last_used_at_the_same_moment_keep_the_order_they_were_first_seen` pins it (an alphabetical tie-break is
   caught). No code change.
3. **`shortcut_keys`' 8.** It is `ROUTE_GROUP_LIMIT + 2` (`status.ts` `recentRouteRows`: "a list short enough to see at once"),
   and the `take(4)` is `RECENT_ROUTE_LIMIT`. Both constants now live in `core/src/recent_routes.rs` and the picker's own
   `ROUTE_GROUP_LIMIT` (`thread_view.rs`) takes its value from there, so there is one 6. Test:
   `a_list_of_eight_is_short_enough_to_take_in_at_once_and_nine_is_not` (broken: `+ 3` is caught).
4. **Paper Mono's width.** Measured from the font file: every glyph in Paper Mono advances 0.606 em (606/1000; checked the
   first 300 glyphs and the usual punctuation); Geist Mono 0.600 (a few glyphs 0.500). The only fixed cell width in the
   Rust window is the terminal's `7.2` (12 pt x 0.6, `terminal_panel.rs` lines 321, 432, 458, 460), which is Geist Mono's
   exactly. But the terminal draws in `Family::SystemMono`, which Rune resolves to the installed system monospace first
   (`rune/src/text.rs` ~379) and to the app's code font only as a fallback, and each cell is its own fixed-width box, so
   Paper Mono's width never reaches it and there is no drift. Nothing to fix. If the terminal is ever set in the picked
   code font, a Paper Mono cell at 12 pt is 7.27, not 7.2. (Not checked: whether Paper Mono's weight axis moves its advance;
   the default master is what was read.)
