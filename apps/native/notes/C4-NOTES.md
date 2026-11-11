# C4: faces (11 Nov 2026)

A teammate's face in Electron is its bot (shared/avatar.ts), drawn by TeammateBot on a canvas, with terminal faces on
and plastic shading by default (botLook.ts). The port shows the same bots, as stills.

## What was built

- **`scripts/bot-stills.mjs`** bundles Electron's own TeammateBot with its stylesheet, draws every bot a seeded look can
  come to (the twenty derived shapes, both faces, the eleven hues: 440) still and idle at 96 pixels in a hidden window
  on a transparent ground, crops each into `assets/bots/<shape>-<face>-<hue>.png`, and writes `src/look/bots.rs`,
  which compiles them in (2.9 MB; the binary is 43.5 MiB).
- **`src/look/faces.rs`** ports the rule: `seed_parts` is seedAvatar (FNV-1a over each code point's first UTF-16 unit,
  the avalanche, a byte window per part), `bot_for` is botFor, and a teammate's recorded look (its hue, its stored parts,
  a bot it picked) wins over the seed. `face` draws the still at a size; `tile` is Home's `.lc-hometeam__tile` (46
  square, radius 10, the hue at 11% over the raised fill, edged at 20%, the bot 34 high).
- **`native::parse_looks` / `load_looks`** read each teammate's hue and avatar beside the roster, from the same file.
- **Where faces show**: the sidebar's teammate rows, Home's team cards on their hue tiles, and the thread's head.

## Checks

- `faces_match_electron_for_a_hundred_ids`: `scripts/face-goldens.mjs` runs Electron's own avatar.ts (bundled with
  esbuild) over 100 ids, including a UUID, digits, accented and CJK names and an emoji, into `parity/faces.json`; the
  Rust rule gives the same parts, shape and face for every one.
- Every derived bot has a still in every hue; a picked shape with no still (critter, prompt, spark) falls back to its
  derived bot rather than drawing nothing; a recorded look wins over the seed.
- Snapshots of Home and the thread in both themes, compared by eye with Electron's Home.
- Tests: 422 in the engine, 190 in the window, all passing.

## Not done

- Bots are still: blinking, glancing, working and finishing come in Phase D.
- Pets, Plush and terminal faces off draw as the default bot; picked shapes outside the twenty need stills of their own.
- The light theme uses the dark-theme stills (Electron draws the bots the same in both).
- `bot-stills.mjs` needs Electron's desktop dependencies, so CI does not regenerate the stills; the face rule itself is
  checked in CI by the goldens test.
