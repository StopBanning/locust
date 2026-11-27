# Picture tests: an approved screen keeps its look (Shots 1, 26 Nov 2026)

Branch `exec/native-shots-1`. Why: on 26 Nov the owner drove the Rust port beside Electron and found screens that every
test called "carried" but that looked wrong. Behaviour is locked by tests; the look was not. Now a screen approved by
eye keeps its render as a reference picture, and a picture test fails when that render changes unasked.

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
