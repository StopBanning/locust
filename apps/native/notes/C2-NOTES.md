# C2: icons and marks (11 Nov 2026)

## What was built

- **`scripts/marks.py`** writes `src/look/icons.rs` from Electron's own drawings, so the two apps draw the same glyphs:
  - the 55 icons of `components/Icon.tsx` (24 box, 1.7 stroke, round caps and joins; Lucide's under ISC and Locust's
    own), each element given the stroke the React wrapper applies;
  - the 10 agent marks of `runtimeMarks.ts`, each in its own box, centred in a square as `rune::Icon` takes one;
    Gemini's gradient carries its stops from `tokens.css` (the same in both themes), drawn as full SVG;
  - the Locust mark and wordmark from `assets/`, in the text's colour.
  `python3 scripts/marks.py --check` fails when Electron's drawings change and the Rust file has not been rewritten;
  the CI scoreboard job runs it.
- **Builders in `look::controls`:** `glyph` (an icon in the control colour), `agent_mark` (Claude's in
  `--lc-mark-claude`, the rest in the words' colour, as `RuntimeMark.tsx`), `lockup` (mark over wordmark), `brand_row`
  (`.lc-brand__lockup`: mark 24 high beside wordmark 13 high) and `head_icon` (`.lc-headicon`: a ghost icon button
  whose help is its tooltip and whose label is its screen-reader name).
- **No toolbar is a row of words:** the thread's Browser, Terminal, Open in terminal and Catch up terminal labels are
  icon buttons now (globe, terminal, code and clock, as Electron's `BrowserToggle`, `TerminalToggle`,
  `OpenInTerminal` and `TerminalDivider` draw them), each with a tooltip and a name.
- **The sidebar's head** draws the Locust mark and wordmark where it wrote "LOCUST".
- **`--scene icons` / `icons-light`:** every icon, every agent mark and the lockup.

## Checks

- `every_drawing_draws`: each icon, mark and Locust drawing covers pixels at 32 px, so an SVG feature Rune's icon drawer
  skips fails as a test, not as a blank glyph. (Gemini's gradient is drawn as full SVG, not as a mask, so it is
  checked by eye in the gallery.)
- Snapshots in both themes, inspected: the gallery, and the thread with its new toolbar and sidebar head.
- Tests: 422 in the engine, 187 in the window, all passing.

## Not done

- The sidebar's teammates still show an initial, not a face (C4), and the brand row does not yet go Home on a click.
- The approval card's buttons, the composer's chips and Settings' controls are still drawn by hand; C3 moves the shell
  onto the kit.
- The marks' sources (`runtimeMarks.ts`) name where each shape came from; that table is not copied into the Rust file.
