# Picture tests: an approved screen keeps its look (Shots 1, 26 Nov 2026)

Branch `exec/native-shots-1`. Why: on 26 Nov the owner drove the Rust port beside Electron and found screens that every
test called "carried" but that looked wrong. Behaviour is locked by tests; the look was not. Now a screen approved by
eye keeps its render as a reference picture, and a picture test fails when that render changes unasked.

## What runs

`src/picture_tests.rs`, in the `locust-native` binary's own tests (`cargo nextest run -p locust-native picture_tests`;
every full nextest run includes them). Each locked scene is rendered **in-process**, through the same
`rune::tooling::headless` path `locust-native --scene NAME --snap out.png` takes, by `picture_render` in `src/main.rs`
(right after `main`), and compared with `pictures/NAME.png`.

The hook builds the same `App` as `main`: setup, poll, keys, fonts, 1215x800, `HEADLESS` on, and the default code font.
The window's glass options, glyph smoothing and frame come from `main_window_look`, which `main` and the hook now both
call. A copy of those three lines in the hook broke two existing tests, which hold `main.rs` to exactly one smoothing
line (`type_tests`) and two `.chrome(WINDOW_CHROME)` (`mac_frame_tests`). Sharing them keeps the counts and means the
hook cannot drift on them; `the_app_window_and_the_picture_render_take_their_look_from_one_place` holds `main()` and the
hook to building their `App` inside `main_window_look`. A look option set on `main`'s `App` anywhere else is **not**
seen by the picture tests (they never run `main`): put it in `main_window_look`.

The hook (`src/main.rs`, after `main`): `main_window_look` (5 lines) and `picture_render` (18 lines), plus
`main_window_look(` around `main`'s own `App::new(AppSpec {` and the `.maximized(..)` line that now closes it.

Locked (8): `settings`, `settings-light`, `people`, `people-light`, `thread-rich`, `thread-rich-light`, `empty`,
`empty-light`. Rendered at `--at 1 --scale 1`: one second in, at 1x. 1x keeps the eight references at 756 KB in all.
2x would be ~4x that, and 1x still shows a few pixels' move or a recoloured label.

**The time is checked, not left to the pixels.** Each render reads its own snap time back from its `--tree` dump (`t`),
and `check` fails a render not taken at `AT` before comparing a pixel. The pixels could not tell: in-process, every
locked scene is already settled at frame 0 except the title bar's green working dot (6x6 at 1126,16), which pulses on
Rune's frame clock (opacity 0.35 at 0 s, 0.84 at 1 s), plus a second pulse in `thread-rich`. That is 32 pixels
(96 in `thread-rich`), under the 0.02% share, so a render at 0 passed all eight picture tests (row 1's break run,
27 Nov). `a_render_is_taken_at_the_locked_time_and_one_at_zero_fails` shows both halves.

## Approving a render

1. Build the release binary: `python3 scripts/gentle-cargo.py build --release`.
2. Render the scene and look at it, e.g. `python3 scripts/isolated-test-runner.py <target>/release/locust-native --scene
   people --snap /tmp/people.png --at 1 --scale 1`. When a test failed, it already left `pictures/NAME.new.png` (the
   new render) and `pictures/NAME.diff.png` (the reference dimmed, changed pixels in magenta) to look at.
3. If it is right: `scripts/picture-lock.sh people` (several names at once are fine). That re-renders into
   `pictures/people.png` at the tests' `--at 1 --scale 1`, and removes the `.new` and `.diff` files. Commit the PNG
   with the change that moved it. It refuses (exit 2, nothing drawn) any name that is not a `LOCKED` line; an
   `UNLOCKED` key such as `settings-meta`, or a prefix such as `sett`, is not one. Two tests run the real script in a
   scratch copy of the tree with a stand-in runner, so they never touch `pictures/`.

The test never writes a reference; only the lock script does, and only for scenes in `LOCKED`. To lock a new scene,
add it to `LOCKED` with a test that calls `holds("name")`, then lock it. `every_locked_scene_has_a_test_and_an_approved_picture`
fails until both exist.

The lock script renders through `scripts/isolated-test-runner.py`: the empty HOME and data folders every test gets.
That is not optional. With the real HOME, `Store::local()` seeds `teammates.json` from this Mac's Electron folder. Its
settings say `codeFont: "paper"`, so a plain `--snap` drew every mono line (model names, chips, the RECENT and RUNS
labels) in Paper Mono, not the default Geist Mono. The first references were drawn that way, and every scene failed
against them by 0.3-0.6%. **Any `--snap` run with the real HOME shows the owner's own code font, not the default.**

## The tolerance

A pixel counts as changed when its red, green or blue moves by more than **8 levels** (of 255). A scene fails when more
than **0.02%** of its compared pixels changed (~190 of 1215x800).

Why: anti-aliasing noise moves an edge pixel by a few levels, so 8 lets it through. Any real recolour moves every pixel
it covers by more. The 0.02% share lets a few strays through, but not a moved, recoloured or missing element; one short
label already covers several hundred pixels. In practice, renders of the same scene matched within 1 level outside the
unlocked elements below.

**One element is left out of the comparison** (`UNLOCKED`). Its rectangle is read from the render's own `--tree`
by element key, so it follows the element if the layout moves it. The left-out area may be at most 2.5% of the picture
(`left_out_too_much`); it is ~1.6%.
- `settings-meta`, the Settings header's "Locust 0.883.0 · local only". It names the version, which every release
  bumps. A debug build (the tests) also adds " · development build" (`cfg!(debug_assertions)`, settings_panel.rs),
  which the release-drawn references never show. The element grows across the header, so its rectangle is the strip
  from after "Settings" to Close (832x19).

The sidebar's working row ("Review the native preview", `conversation-title-sample-2`) was left out too until 27 Nov:
every locked scene is built on the team fixture, whose Argon is working, so this title shimmers, and the band ran on
the wall clock (an `Instant` started at the first shimmer drawn). Where it stood in a snap depended on how fast the
machine drew the 60 frames before it (two runs of one release binary moved ~500 pixels of it by up to 134 levels). The
band now takes its time from the window's clock, `poll`'s `now` (`State::frame_clock`, `working_shimmer::title`). A
scene's view is built only when its state changes, so a snap draws the band where the window's clock stood at the last
build, the same on every run, and the title is compared (harden-1;
`the_working_title_is_compared_and_two_renders_draw_its_band_alike`). A real window builds its view every 50 ms while a
row works, so there the band moves.

## How long it takes

After the build, the 27 tests in `picture_tests` take ~2-4 s under nextest (27 Nov: 1.6-3.4 s; one in-process
debug render is 0.4-1.9 s). They add nothing to the build beyond one more test module. Approving is one release-binary
render per scene, about a second each.

## Scenes not locked, and why

Every one of the 134 scenes (`--list-scenes`) was rendered twice by the release binary under the isolated runner and
compared under the rule above (26 Nov):
- **116 differ only in the shimmering working title** (the box at 49-157 x 323-333). With `UNLOCKED`, they could be
  locked as they are.
- **`reviews-live`** also differs elsewhere: 700 pixels in 49-374 x 229-546, up to 202 levels (a live review's own
  motion). Not lockable until that is on the frame clock too.
- **18 were byte-identical**, most only because the run reached the snap before the band entered the title. The
  `quick-*` Quick Ask scenes, `kit` and `icons` have no working row at all.
- **`--at 0` is not usable for any scene.** With the release binary, frame 0 differed from itself across runs (top
  bar and content still loading: 150-660 pixels) and from the settled frame. In-process it differs from the settled
  frame only by the pulsing working dot (above), which is why the time is checked directly.

The first eight were the ones the brief named. More are a `LOCKED` line, a test and a lock away.

## Breaks (mut6)

Each break was applied alone, the tests run, and the file put back (`~/work/scratch/mut6.py`, 27 Nov). Every one below
was caught by a failing test, not by a failed build.

**Row 3: the look itself.** Each is caught by the picture tests:
- (a) A colour token: `look::tokens` reads `--lc-text-faint` where `--lc-text-muted` is asked for (`src/look/mod.rs`).
  Caught by all eight picture tests (and `look::tests`' palette check).
- (b) An element removed: the team fixture keeps only its first two teammates (`.take(2)` in `src/ui/scenes.rs`).
  Caught by all eight.
- (c) Spacing: a row's gap 9 to 12 px (`src/look/controls.rs`, line 291), caught by all eight; a switch's padding
  2 to 4 px (`controls::switch`), caught by `settings` and `settings-light`, the two locked screens with switches.
- The harness's own tolerance raised to accept everything: `MAX_CHANGED_SHARE` 1.0, caught by `a_recoloured_label_fails`,
  `a_moved_element_fails`, `a_missing_element_fails` and the failure-path test; `CHANNEL_TOLERANCE` 255, caught by
  those and five more.

6 of 6 caught. Rows 1 and 2:
- **Row 1, the harness:** 14 breaks, 13 caught at first. The survivor rendered at `--at 0` and passed every picture
  test (see "The time is checked" above). After the fix, it and four more were run: rendering at 0 (both in `check`
  and in the render's arguments), the time check bypassed, its call in `check_at` removed, and `main()`'s window
  built outside `main_window_look`. 5 of 5 caught.
- **Row 2, the lock script:** a time of 0, a scale of 2, the old loose `LOCKED` grep, a prefix-matching grep, a refused
  name that does not exit, the isolated runner bypassed, the `.new`/`.diff` files left behind. 7 of 7 caught.
