# Home notes

## Home round 1 (19 Nov 2026)

### Part 1 (78152a94): the pure rules, in core
- `core/src/home_cover.rs`: the cover's scale, growth into spare height, the short-Home check and the normal-spacing
  memory. Also `glass_status`, the status line that names who waits on you and who is working.
- `core/src/home_team_state.rs`: each teammate card's state, `last_worked_phrase` and `waiting_line`.
- These are tested in core but Home does not draw them yet. Their rows stay `waiting` until it does.

### Part 2: install first, and the line under the machine
- **Install first** (`with-nothing-installed-install-comes-first`, 4/4).
  - Rust Home had no agents panel. It now draws Settings' own "AI agents & accounts" panel, Install rows included.
  - The panel comes first while discovery says nothing is installed (`core/src/home_order.rs` `accounts_lead`), and
    after the teams and the folder once anything is.
  - An agent still being checked counts as installed.
  - Before discovery answers, Home keeps its usual order rather than jumping when the answer lands.
  - Home asks for discovery when it opens if nothing has answered yet.
  - Not Electron's: an answer naming no agents puts no empty panel first. Electron's `every` is true on an empty list.
- **The line under the machine** (`the-first-screen-line-is-centred`, 2/2).
  - Electron reads its stylesheet; here the drawn frame is measured, which is what Electron's probe on the built app
    does.
  - The line is centred on the lockup within 2px at 1215 wide, sits under it, shows for a first run only, and waits
    for the agents to answer.
  - Its second sentence is a promise about OpenCode's prices: `free_start_still_free` (Electron `freeStartStillFree`)
    withdraws it only when a ready OpenCode lists nothing free.
  - Rune has no per-line text centring. The box is centred, and the line fits one line in the 760 column; a window
    narrow enough to wrap it would centre the box, not each line.
- **A stale sentence fixed.** Settings' none-installed lede said "run its line below", although the row has had an
  Install button for some time. It says "press its Install" again, as Electron does.

### Part 3: the glass and the cards say what is happening
- **The cover's glass** (`homeCoverStatus`, 3/3).
  - While work runs, the glass says who waits on you, then who is working, in place of the claim (`glass_status`).
  - It is fed by the same `teammate_status` the cards below use, so the two never disagree.
- **The team cards** (`homeTeamState`, 10/10).
  - The state word sits beside the name in its tone: the live word, "waiting on you" in amber, or what blocks the
    teammate in red.
  - While they wait, the waiting line (the question as asked, "Wants to run …", or "Wants your approval: …") takes
    the role's place.
  - An idle card says nothing beside the name, and keeps "last worked …" for its hover.
  - The older shared test now expects Home's mid-line "sign-in needed"; Team still says "Sign-in needed".
  - Not drawn yet: the step under way, as the line under a working card.

### Not done
- Electron folds a ready agents list to one head line ("N agents ready"). Rust Home draws the whole panel, so a
  returning Home is taller than Electron's.
- Home drawing the cover rules (scale, growth, fitting one screen):
  - `the-home-screen-is-the-cover`, `home-fits-one-screen`, `home-fits-two-rows-of-teammates`;
  - `home-holds-still-when-tight-spacing-shrinks-it`, `the-cover-grows-into-spare-height`.

## Home round 2 (20 Nov 2026)

### Part 1 (a46e0c68): the cover's pieces, in core
- `core/src/home_cover.rs` gained the machine and its cast on the cover's grid (`COVER_MACHINE`, `COVER_CAST`):
  - the ghost, white and floating;
  - Prompt in blue;
  - Claw'd's critter in clay, asleep.
- `cover_place` puts the cover on a card in whole pixels: 68 across at (213, -7) on the 760 card, as Electron's
  `drawn` / `originX` / `at` do.
- `home_fit` is FirstLaunch.tsx's measure step as one function: tight or not, and how much the cover grows.
- `core/src/cover_script.rs` is coverScript.ts: the worker's 17 s loop, the thinker's 13 s, `beat_at` with the time
  to the next beat, and a `CoverClock` that banks while held.

### Part 2 (e6b6375c): Home draws it
- **The cover** replaces the fixed glass box:
  - The bezel and glass are placed at the card's scale; the lockup and claim sit on the glass under a scan layer.
  - The three stand on the bezel, hidden from screen readers. The machine keeps the `Locust. …` label.
  - Before the agents answer, all three are still, with no z's and no float.
- **The ghost's float** is Rune's own looping slide on lcBotFloat's curve (`Easing::Piecewise`). The renderer runs
  it, so it costs no view. It is smooth, where Electron steps it in 11.
- **The beats** follow the cover's clock.
  - An empty mark is keyed by the two beats and by whether the cover plays. Each new key is laid out once, and that
    is where the clock starts or is held, and where the one wake for the next change is asked for.
  - So the view never runs on a tick. `poll` lives in main.rs, which is not this lane's.
- **Home measures itself** with `on_layout` on its pane, the cover's card and its column. Each handler says
  `live_only` when nothing moved.
  - The sections are the column's height less the padding, gaps and the cover's footprint it drew. So the cover's
    growth never moves its own room (Electron's 0.516 stutter).
  - Rune springs the size change where Electron jumps.
- **Tight spacing is drawn:** 12px for 32, and 8px gaps for 12. These are the spacing numbers core decides with.
  Home's column used to be `pt(18).pb(28).gap(12)`, which no rule knew.
- **Scout's card:** the eye is named "Scout: look now", and its title says Look now or Looking…. The watch line keeps
  the role as its hover.
- **Kept from Electron:** the footprint is counted without COVER_HOME_SCALE 0.9, so the cover is drawn 10% smaller than
  the measure counts it. Home tightens and gives back slightly early and keeps the difference as air. The pinned
  Electron numbers stay true.

### Breaks (24, each caught by a named test)
- **Core (11):** the worker's loop length, the thinker's eyes, the done beat's hop, the time to the next beat, the
  clock's banking, the critter's clay, the ghost's float, the cover's origin, deciding at the normal spacing, the
  growth, and the claim's floor.
- **Home (13):** the column's sections, tight spacing drawn, the growth drawn, the scan layer's place, the float, the
  z's before the cover is on, the bots hidden from screen readers, the eye's name, the watch line's hover, reduced
  motion's still beats, the beat names before the cover is on, and the clock starting.
- Counting the cover in its own sections first survived. `the_cover_takes_the_spare_height_and_leaves_only_the_air`
  now catches it: a cover too small still fits, so only the leftover space shows it.

### Not drawn, and not checkable headless
- The hop and the glance a beat starts with. The rig's lift would hold for the whole 1.8 s beat and read as a float,
  so `hop` is drawn false.
- The rig's look-about, the lockup's power-on, and the loading beam.
- The sleeper waking for a pointer (`hopperWakes`, `WAKE_WITHIN`, `DOZE_AFTER_MS`).
- The clock resting after 45 s untouched. It holds only while the window is unfocused or motion is reduced; the
  45 s rest needs `cover_watch::away` with main.rs's poll clock.
- The claim stepping aside when the glass is narrower than it. It is drawn, at alpha 0 and out of the layout, but
  no test makes the glass that narrow.

### Not done
- **The accounts between the cover and the team** (home-fits-one-screen case 1). Tried, and taken back:
  - Drawn above the team, Scout's comment Send lost its press in
    `window_drive_tests::scout_comment_ask_sends_and_posts_reviewed_reply_from_window`. `send_draft` never ran.
  - Bisected through `scripts/isolated-test-runner.py`, with temporary switches: the order alone passed. The order
    with the cover, or with the new spacing, failed. So it depends on where the button lands.
  - The button was on top at the press point, its hit target matched its rect, nothing in the tree changed between
    frames, and no modal was open. Cause not found.
  - **Repro:** set `accounts_above = accounts_lead || !s.teammates.is_empty()` in `home_view` and run that test.
  - If the lost press is real, Scout's comment Send would do nothing for a person. That blocks the flip.
  - Note: a raw test binary refuses to run outside the isolated runner (core/src/test_isolation.rs). Run a bisect
    through `python3 scripts/isolated-test-runner.py <binary> <filter>`, never the bare binary.
- **Home fitting one screen.** Measured headless:
  - At 1205x803 with Codec, Argon and Scout, the column is 968px tight in an 803 pane. Its other sections are 673px
    (Electron 326).
  - At 1209x782 with six teammates, it is 1054px in 782. Its sections are 759px.
  - Mostly this is the whole agents panel, where Electron folds a ready list to one line. The folder card above the
    team, the close row and the starters' lede add the rest. The rule tightens and floors the cover, but the page
    still scrolls.
  - home-fits-two-rows stays partial on that outcome.
