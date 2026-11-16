# The Teammate look picker and the face legend (Opus, round 4, 15 Nov 2026)

Branch `exec/opus-bots`, from native-engine 46237ec5. Parity row 3, "Custom faces, colours, bot/mascot and status
legend" (`missing` before this round).

## First, a correction to the hand-off

The hand-off names `LookSetting.tsx` as Electron's teammate look picker. It is not one. `LookSetting.tsx` is Settings
› Appearance's **window** look: Standard or Liquid glass, and "Behind the glass" (its test is
`the-look-is-offered-only-where-it-can-be-drawn`). That needs macOS's glass, and it is not this job.

Electron's teammate look picker is the **Look** section of `NewTeammateDialog.tsx`, lines 509-746. The same dialog
is both "New teammate" and "Edit teammate". It is read below, and that is what this round ports.

## What Electron's picker does (NewTeammateDialog.tsx, shared/avatar.ts, main/teammate-store.ts)

**The look is generated, then owned.**
- When the dialog opens for a new teammate, it seeds a look from a throwaway id:
  `seedAvatar('draft_' + Date.now() + '_' + Math.random())`. So two dialogs opened in a row start on different faces.
- When editing, the dialog starts on the teammate's own avatar. The store gives `seedAvatar(teammateId)` for a record
  without a valid one (`teammate-store.ts:442`).
- Whatever the preview shows when the person presses Create or Save is what is saved (`avatar` in the draft). The
  look never comes from the name, so a rename does not change a face.

**The preview.** At the top beside Name: `TeammateBot` at 64, working, with the working dot, at full motion.

**The colour swatches.** Under the name, a radio group called "Avatar colour". It holds 11 colours in this order,
each labelled: Lime, Blue, Violet, Clay, Teal, Butter, Rose, Slate, Pearl, Indigo, Coral.
- Each swatch is the teammate's own bot, at 24, in that colour.
- While a pet is worn, each swatch is a plain colour chip instead.
- A new teammate starts on `freshHue` (already ported: `people::fresh_hue`).
- The free name in the empty box is `suggestedName` (already ported: `people::suggested_name`).

**The Look section.** It sits after the Model row. Its head is the label "Look", then the Face choice, then
**Shuffle look**.
- **Face** is a radio group with three choices: Eyes, Mouth and Screen. It is hidden while a pet is worn.
  - Terminal faces is Settings › Appearance's switch. Prompt and Spark always wear their screen.
  - So Screen is off while Terminal faces is off, unless the shape is Prompt or Spark. Its title then says "Turn
    Terminal faces on in Settings > Appearance to give a teammate a screen."
  - Eyes and Mouth are off for Prompt and Spark, titled "Prompt's face is its screen."
  - The choice shown is Screen when the bot wears one, worked out as `alwaysScreen || (terminal && (bot.screen ??
    screenSuits(shape)))`. Otherwise it is the bot's face.
  - Pressing Screen saves `{shape, face: kept, screen: true}`. Pressing Eyes or Mouth saves `{shape, face, screen:
    false}`.
- **Shuffle look** is `shuffledAvatar`: each of the three parts steps on by one, and the picked bot and the pet are
  dropped. So the look goes back to a derived bot.
- **The shape grid** is a radio group called "Look". It holds all 23 shapes in `BOT_SHAPES` order, each drawn still
  at 30 in the teammate's colour, with the face as chosen so far (`withShape` keeps face and screen).
  - The labels are `SHAPE_NAMES`: Clover … Puddle, "Hopper, a Locust", "Swarm, a Locust", Critter, Spark, Prompt.
  - Picking a shape takes off any pet.
  - A shape shows as chosen only while no pet is worn.
- **A worn pet's face.** When the worn pet's drawings have a screen or a puppet, a second Face group offers
  "As drawn" or "Screen" (Screen only while Terminal faces is on). Pressing one saves `pet.screen`.

**What the store does with it** (`teammate-store.ts` 740-805):
- **Create.** No avatar means `seedAvatar(teammateId)`. An invalid one is refused with "Teammate avatar is invalid".
  A valid one is cleaned (`cleanAvatar`: only the known fields reach the disk).
- **Update.** An avatar must be given and valid, and is cleaned.

## What this round builds in Rust

1. **The look on the form.** `people_panel`'s form is the new-teammate dialog and the editor in one, as Electron's is.
   - It keeps the look being edited as an `AvatarSpec` (`core/src/bot_spec.rs`).
   - A new teammate is seeded from a draft id made of the time and a random number.
   - An edit starts from the record's avatar, or `seed_avatar(id)` when the record has none.
   - Create and Save write it as `avatar`.
2. **The store** (`people::save_teammate_id`):
   - It refuses an invalid avatar with Electron's words, cleans a valid one, and seeds a new teammate without one from
     its id.
   - **Different from Electron:** an update with no avatar keeps the one on the record rather than being refused. Six
     other Rust callers (instructions card, thread view, compare, home, team card, routines import) save a teammate
     without going through the picker, and they carry the record's own avatar.
   - The avatar's JSON goes through `avatar_from_json` and `avatar_json` in `bot_spec.rs`. They are the Rust form of
     `isAvatarSpec` and `cleanAvatar`.
3. **The picker in the form:**
   - the live preview, from `faces::bot_face_with_look` at 64, working, with the working dot, at full motion;
   - the 11 labelled colour swatches as the teammate's bot, or as chips while a pet is worn;
   - the Look head (Face, then Shuffle look) and the 23-shape grid, with Electron's labels, order, disabling and
     titles (as `help`).
   - Each choice is a radio with its checked state for a screen reader.
4. **A worn pet.**
   - When the worn pet is Codex Buddy (the one pet the Rust app draws), the "As drawn" / "Screen" pair is shown.
   - The Face group steps aside and the swatches become chips, as Electron does.

## What this round does not build (and why)

- **The Pets row** (PetPicks tiles, the gallery picks, Codex pets read from disk, removing pets, the credit line,
  "could not be found"). The Rust app draws one pet, Codex Buddy, and has no pet list or pet store to choose from. That
  is its own parity work (`a-teammate-can-be-a-pet`, `the-kept-pets-are-puppets`).
- **The quickest way in.** Electron fills an empty name with a picked pet's own name. There is no pet list to pick
  from, so this is not built either.
- **The layout.** Electron's dialog puts the preview and the swatches beside the name, then Model, then Look. The Rust
  form is a single column of fields, and that stays as it is. The look sits under the fields, before Create.

## The legend: "What a face says" (FaceLegend.tsx), against what Rust had

- **Electron.** It is one button: a droid in teal at 40, with eyes. It goes through eight faces in turn, each for its
  own time: Idle 2.6 s, Thinking 3.4, Working 3, Replying 2.8, Done 2.4, Waiting on you 3, Listening 2.6, Blocked 2.8.
  - Each face wears its dot (`legendPresence`): working for thinking, working and replying; amber for waiting; red for
    blocked; none for the rest.
  - The name of the face sits under it, and its label is "A teammate's face: {name}. Show the next one."
  - A click shows the next face.
  - It rests (stops where it is) behind other windows, when hidden, when untouched for 45 s, or with reduced motion.
- **Rust, before this round.** It drew six faces side by side: a blue circle at 26, at subtle motion, with no dots.
  - The words were "Resting", "Thinking", "Working", "Waiting", "Done" and "Blocked". "Replying" and "Listening" were
    missing, "Resting" should be "Idle", and "Waiting" should be "Waiting on you".
- **This round.**
  - The eight steps and their times move to core (`face_legend.rs`), with the names from `bot_rig::face_label` and the
    dots from `face_status`.
  - The window draws one face that steps through them as Electron's does. A press shows the next.
  - It rests while the window is not focused or reduced motion is on. The 45 s untouched rest is Home cover's rule
    (`useCoverActivity`), which the Rust app does not have yet. Until it does, the legend keeps going while the window
    is focused.

## Verification: the picker (step 2)

### Tests (one case, one test)

- **Core, `look_picker::tests`** (11 tests). They cover the shape names and colour names in order, which face shows as
  chosen, why Screen or Eyes and Mouth are off, what pressing a face gives, what a shape keeps and takes off, Shuffle,
  a worn pet's face, draft ids, reading an avatar as `isAvatarSpec` does, and writing only the known fields.
- **The store, `people::tests`** (5 tests):
  - `a_new_teammate_without_a_look_is_seeded_from_its_own_id`
  - `the_look_picked_is_saved_cleaned_to_its_known_fields`
  - `a_look_that_is_not_one_is_refused_in_electrons_words`
  - `an_edit_saves_the_new_look_and_one_without_a_look_keeps_the_records`
  - `a_bad_look_already_on_the_record_is_carried_as_electron_reads_it`
- **The window, `look_picker::tests`** (11 tests). They draw the real form and read its accessibility tree.
  - Electron's three dialog cases, by name: `offers_every_shape_to_choose_from_and_the_face`,
    `keeps_shuffle_once_with_the_look_it_shuffles` and `has_eleven_colours_indigo_and_coral_drawn_as_the_teammates_own_bot`.
  - The preview.
  - The opening look.
  - A drive that picks a face, then a shape, then a colour, presses Create and reads the saved record.
  - Screen and the mascots' disabled choices, with their titles.
  - Pressing Screen and Eyes.
  - Shuffle.
  - A worn Codex Buddy.
- The old colour test (`people_panel::renderer_defaults_tests::is_where_the_dialog_opens_…`) now reads the checked
  radio, "Blue" or "Rose", in Electron's words. It used to read the lowercase "blue selected".

### Break checks

All **16 were caught by a named test** (`~/work/scratch/mut6.py`, whole package, one per command; log in
`notes/look-breaks.log`): 10 in core and 6 in the window.

- **A test the picker broke.** Under the first window break,
  `window_drive_tests::own_copy_is_turned_on_in_the_card_and_a_copy_is_removed_only_after_its_changes_are_named` also
  failed. It failed on the unbroken tree too. The look section makes the form taller, so the Own copy switch was below
  the window and the click missed it. The test now scrolls to the switch before pressing it, as it already did for
  Save. It passes.

### Full run and build

- **Full release workspace run** (`notes/look-suite.log`): **2,190 engine + 528 window passed**, 0 failed, 20 ignored.
  Every test named above passed by name.
- **Release build:** passed.

## Verification: the legend (step 3)

### What was different, and is now Electron's

- **Faces.** Rust showed six faces side by side. Now one droid goes through Electron's eight in turn.
- **Words.** "Resting" is now "Idle", and "Waiting" is now "Waiting on you". "Replying" and "Listening" were missing
  and are now there.
- **Look.**
  - A blue circle at 26 is now a teal droid at 40, with eyes. Like Electron's, its screen is left unset, so it wears a
    screen while Terminal faces is on.
  - It moves at Subtle (TeammateBot's default), as before.
- **Dots.** There were none. Now each face wears its dot: white while working, amber while waiting, red when
  blocked.
- **The row.** It now has Electron's words beside it (`legend_lede`), which change with Terminal faces.
- **Pressing.** A press shows the next face.
- **Resting.**
  - It rests while the window is not focused or motion is reduced.
  - It starts again from Idle when Appearance is shown again, because Electron's component mounts afresh.

### Tests and checks

- **`the-face-legend-shows-every-face.test.tsx` is carried, 5 of 5.**
  - Four cases are in core, `face_legend::tests`: every face once, the names, the dots, and the times.
  - The fifth is in the window: `settings_panel::tests::face_legend_tests::is_a_button_that_says_which_face_it_shows_the_face_under_it_a_real_teammate_face`.
- **Behaviour tests for the timer**, each one a behaviour of FaceLegend.tsx:
  - core, 4 tests: the timer, resting, going round again, and the lede;
  - window, 3 tests: a press, each face's time with the amber dot, and resting then starting from Idle.
- **Break checks:** all 6 legend breaks were caught by a named test (`notes/look-breaks.log`). The core breaks `c09`
  and `c10` above also apply to the legend.
- **Full release workspace run** (`notes/legend-suite.log`): **2,191 engine + 532 window passed**, 0 failed, 20
  ignored. All 12 legend tests passed by name.
- **Release build:** passed.
- **Ledger** (`recount.py --check` passes):
  - Row 3 moves from `missing` to `partial`. Rows are now 71 done, 66 partial, 16 missing.
  - Test files: 202 carried, 192 partial, 853 waiting.
  - `every-teammate-is-a-bot` is 11 of 14 (step 2).

## The pictures (step 4)

### How they were made

- **Scenes.** `team-look` and `team-look-light` are new (`src/ui/scenes.rs`). They show the editor open on Codec, a
  violet ghost with a mouth and its screen turned off, with Terminal faces on.
- **Settings.** `settings` was already a scene, and it shows Appearance.
- **Command.** `locust-native --snap ~/work/scratch/opus/<scene>.png --scene <scene>`, headless, with no window.
- **The form's order changed.** In the first picture, the Look section was below the fold, under Folder. Electron puts
  Look before Role. So it now comes straight after Name (and the name-taken line) and before Role. That is closer to
  Electron, and it is on screen.

### What I see in `team-look.png` (dark)

- **The top row.** On the left is the preview: the violet ghost at 64 with its mouth and the white working dot at its
  lower right. To its right are the eleven swatches. Each is the same ghost at 24, in Lime, Blue, Violet, Clay, Teal,
  Butter, Rose, Slate, Pearl, Indigo and Coral. Violet has a box around it.
- **Then** come Name with "Codec" in it, and the Look head: "Look" on the left, and on the right the Eyes / Mouth /
  Screen segment (Mouth raised), then a Shuffle look button.
- **The grid.** The 23 shapes are drawn in violet with the chosen mouth. There are 18 in the first row and 5 in the
  second (Hopper, Swarm, Critter, Prompt, Spark).
  - The Ghost tile is the chosen one.
  - Critter shows its mouth, because the screen is turned off.
  - Prompt and Spark show their screens, as they always do.
- **Then** Role, Instructions and Runtime, as before.

### What I see in `settings.png`

- On the right of "What a face says" is the teal droid with its screen eyes (Terminal faces is on), and "Idle" under
  it.
- On the left is Electron's sentence for Terminal faces on.
- Only one face is in a still picture. The stepping is held by the tests, not by the picture.

### What looks off, said plainly

1. **The preview's ghost has no hem.** At 64 it looks cut flat along the bottom, and the dot sits over that corner.
   The ghosts at 24 and 30 show their wavy hem.
   - It may be the working pose (the live rig bobs and squashes), or the canvas clipping the overscan.
   - I did not find out which.
2. **The chosen tiles are grey, not lime.** The chosen swatch and the chosen shape are marked with a grey box and fill,
   not a lime one, although the code asks for `lime-border` and `lime-tint`.
   - Those tokens may be grey in the current theme (Locust went monochrome in 0.430).
   - I did not check Electron's picture against it.
3. **The layout is not Electron's.** Electron puts the swatches under the Name field, beside the preview. Here they are
   on a row of their own above Name. The fields stay a plain column, and Role and Runtime are text fields, as they
   were before this round.
4. **The legend sits far to the right.** It is at the right end of a wide row, away from its words. Electron's
   setting line does the same, but at Electron's narrower width it sits closer to them.
- **Verified after the reorder.** Full release workspace run (`notes/snap-suite.log`): **2,191 engine + 532 window
  passed**, 0 failed. All 22 `look_picker` tests and the Own copy drive passed by name. Release build: passed.

## Round 5, job 1: the picker's rough edges (16 Nov 2026)

1. **The ghost's hem at 64.**
   - **The cause** was the pose, not clipping or the size. Electron's `Bot.tsx` draws a new bot once before its clock's
     first step (`draw()`, line 2007), so the first frame is the rig as `BotAvatarSim` made it, upright.
   - Rust stepped the rig with `update(0)` while making the first snapshot. A working bot starts its hop at phase 0
     (both libraries set `hopPhase = 0` on entering working), and phase 0 is the landing squash: 1.16 wide and 0.82
     high. A still picture is that one frame, so the ghost looked flattened and its hem nearly vanished.
   - **The fix.** `first_bot_snapshot` (`src/look/bot_stage.rs`) draws the first frame without the step, and the clock
     steps it from the next frame on.
   - **Tests:** `first_frame_tests::a_new_working_bot_is_first_drawn_upright_as_its_rig_was_made` and
     `its_clock_then_steps_it_into_the_hop`.
   - **Break check:** stepping the first frame again was caught by the first test, by name.
   - `team-look.png` now shows the ghost upright with its hem.
2. **"Grey, not lime" is Electron's look, not a bug.**
   - In Electron's current theme the `--lc-lime-*` tokens are ivory: `--lc-lime-tint: rgba(242, 239, 233, 0.07)` and
     `--lc-lime-border-strong: rgba(242, 239, 233, 0.38)` on dark. Locust went monochrome in 0.430.
   - Rust reads that same `tokens.css` at build time, and uses the same tokens for a chosen tile:
     - the swatch: `lime-border-strong` and `lime-tint`;
     - the shape: `lime-border` and `lime-tint`.
   - Nothing changed here. No test was added, because `tokens_match_the_css` already holds the values.
3. **The swatches sit beside the preview, under Name**, as in `.lc-dialog__identity` (`NewTeammateDialog.tsx` 511-559).
   - The preview is on the left. Beside it is one column (gap `space-4`) of Name, its box, the name-taken line and the
     eleven swatches.
   - The Look section follows, then Role and the rest.
   - **Test:** `look_picker::tests::puts_the_swatches_beside_the_preview_under_the_name_box`.
   - **Break check:** putting the swatches back above Name was caught by that test.
- **Full release workspace run** (`~/work/scratch/opus/r5-j1-suite.log`): **2,196 engine + 545 window passed**, 0 failed.
  All three tests passed by name. **Release build:** passed.
