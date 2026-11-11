# C1: the kit (11 Nov 2026)

Phase C's first round: one module that is Locust's look, so no screen draws a control by hand again.

## What was built

- **`src/look/mod.rs`.** Electron's `renderer/src/tokens.css` is compiled in (`include_str!`) and parsed: the `:root`
  block for dark, with `:root[data-theme='light']` laid over it for light, `var(--lc-…)` resolved, colours read as a
  browser reads them. The CSS stays the single source while Electron ships, so a colour changed there changes here on
  the next build. `kit(dark)` maps the tokens onto rune-kit's `Kit` (palette, status colours, radii, elevation, type),
  the way rune-kit's own OpenCode theme does. `locust_type()` adds Locust's own roles: the wordmark-style title, the
  tracked mono label, mono captions, a reply's prose.
- **`src/look/controls.rs`.** Locust's simple controls, each drawn to its rule in `shell.css` and named beside it:
  button (neutral, active, danger, disabled), the primary button, chip, card (plain, amber, red, blue), switch, field
  (with the focused treatment), dialog, title, label. Each focusable control has a focus ring and a role and name for
  screen readers.
- **From rune-kit, styled by the kit:** icon buttons, menu, select, tooltip, toast and the segmented control. They
  bring keyboard handling and overlay placement.
- **`src/look/gallery.rs`** and the scenes `kit` and `kit-light`: every control in one frame.

## Checks

- `look::tests` (5): every token the module names exists and reads as the kind it is used as, in both themes; the two
  themes differ where the CSS says; `rgba()` reads as a browser reads it; the kit carries Locust's values; and the
  older hand-copied `Palette` in `ui/app_state.rs` still agrees with the CSS, colour for colour, until the screens move
  onto the kit.
- Snapshots, headless (no window opened), inspected by eye in both themes:
  `locust-native --snap ~/work/scratch/c1-snaps/kit.png --scene kit --size 1200x900` (and `kit-light`).
- Three faults found in the first frames, fixed: the light scene drew dark (a scene must set `demo`, or setup loads
  the saved appearance over it); every shadow was empty (rune-kit's shadow parser takes hex only, so `rgba()` is
  rewritten first); card text never wrapped (a row sizes to its text; the body is a column now).
- Tests: 422 in the engine, 185 in the window, all passing.

## Fonts, the same day

Colin said yes to downloading fonts. Figtree (the variable face and its italic, from erikdkennedy/figtree at `032dfa7`,
the commit Electron's woff2 came from) and Geist Mono (vercel/geist-font at `ef2597c`) are vendored as TrueType under
`assets/fonts/` with their OFL licences, sources and checksums, and both windows load them (`look::FONTS`,
`look::FAMILIES`). Every screen now draws in Figtree, with Geist Mono for mono text, instead of Rune's built-in Inter.
The binary grew by about 0.9 MB. `the_fonts_are_the_faces_the_app_names` checks each file is TrueType and names the
family the app asks for. Before-and-after frames of the gallery and real screens were compared by eye.

## Not done

- **Anthropic Sans** for a teammate's reply: Electron vendors it from a third party's MIT repackage, so its licence is
  checked before it goes into a shipped binary. Replies use Figtree until then.
- **No picture pair against Electron yet.** That is C5's job, with `_tools/polish-frames.mjs` capturing Electron
  headlessly at the same size.
- **No screen uses the kit yet.** Seven screens each still define their own text `button`; C3 moves the shell onto the
  kit and C5 the rest, each with its picture pair.
- Hover is a fill over the control; Electron also lightens the border on hover, which a Rune hover colour cannot do.
