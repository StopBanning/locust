# Rune: the pin, the kept copy, and moving the pin

The window is built on [Rune](https://github.com/anomalyco/rune), a Rust UI toolkit by the OpenCode team. It is young
(first public commit 22 Oct 2026), changes every day, and does not take pull requests. So the app pins one commit and
keeps a copy of its source.

## The pin

`Cargo.toml` takes `rune` and `rune-kit` (features `markdown` and `mermaid`) from git at
`39301f6793266450a057f270da06b048a689be96`. `Cargo.lock` also takes `rune-mermaid`, `rune-motion`, `rune-staging` and
`rune-taffy` from the same commit. All six move together.

## The kept copy

A full mirror of Rune's repository is kept on the Mac mini at `~/Archive/rune-mirror-2026-11-11/rune.git` (taken
11 Nov 2026, 178 MB; it holds the pinned commit). If Rune's repository disappears or rewrites history, the app still
builds from it:

```bash
R=~/work/scratch/rune-from-mirror
mkdir -p "$R"
git -C ~/Archive/rune-mirror-2026-11-11/rune.git archive 39301f6793266450a057f270da06b048a689be96 | tar -x -C "$R"
```

then add to `Cargo.toml`:

```toml
[patch."https://github.com/anomalyco/rune"]
rune = { path = "<R>/crates/rune" }
rune-kit = { path = "<R>/crates/rune-kit" }
rune-mermaid = { path = "<R>/crates/rune-mermaid" }
rune-motion = { path = "<R>/crates/rune-motion" }
rune-staging = { path = "<R>/crates/rune-staging" }
rune-taffy = { path = "<R>/crates/rune-taffy" }
```

Checked on 11 Nov 2026: with that patch, `cargo build --release --offline` built the app in 1 min 47 s with no network
(crates.io dependencies came from the local cargo cache).

To refresh the copy before moving the pin: `git -C ~/Archive/rune-mirror-2026-11-11/rune.git remote update`.

## The local patch

Since 23 Nov 2026 the app builds `rune`, `rune-motion`, `rune-staging` and `rune-taffy` from a copy of the pinned
commit at `vendor/rune` (repository root), through a `[patch."https://github.com/anomalyco/rune"]` table at the end of
`Cargo.toml`; `rune-kit` and `rune-mermaid` still come from git at the pin, and the patch points their `rune` at the copy
too (`cargo tree -i rune` shows one `rune`). The copy is in two commits, so the patch reads on its own:

1. `Vendor Rune 39301f67 for a local patch (unpatched)`: the archive of the pinned commit, those crates and `procwatch`,
   Rune's `Cargo.lock` and `LICENSE`, its root `Cargo.toml` with the members cut to the five. Its only edit: eight
   made-up test fixtures end in `// public-guard: allow`, so this public repository's commit guard takes them.
2. The patch, two changes, each with a Rune-level test in `crates/rune/src/system_material/tests.rs`:
   - **`SystemMaterial::Sidebar`** (`system_material/mod.rs`): `NSVisualEffectView` with the `.sidebar` material (7),
     behind window, state follow-window (0). Rune had `Popover` (6, always active) and `Glass` only; Electron's Mac window
     is made with `vibrancy: 'sidebar'` and `visualEffectState: 'followWindow'` (`main/window-chrome.ts`), so the
     sidebar goes flat when the window loses focus, as Finder's does. `make_view` (`system_material/macos.rs`) now takes
     the material and state from `effect_view_settings`, so `Popover` is made exactly as before. Test:
     `the_sidebar_material_is_the_sidebar_effect_following_its_window_and_popover_stays_always_active`.
   - **`ElementInfo::material`** (`runtime/elements.rs`; filled in `elements()`, `None` for flight and link entries and
     `ElementInfo::default`): the element's `El::system_material`. Without it nothing outside Rune can see a material
     in a headless frame (it draws the element's fill as a stand-in), so no window test could fail when a
     `.system_material(...)` call is removed. Test: `element_info_names_each_elements_material_and_none_for_the_rest`.

   To get the shaders and font Rune's own lib tests read with `include_str!`/`include_bytes!` (not used by the app's
   build), 22 `.wgsl` files from `rune-fx` and `rune-light` and IBM Plex Mono with its licence from
   `themes/opencode/assets/fonts` sit at their usual paths under `vendor/rune`, with no crate around them.

Run Rune's tests in the copy with `cargo test -p rune --lib system_material` from `vendor/rune` (its own workspace).

**To offer it upstream:** both changes are additive; the diff of the second commit under `vendor/rune`, without the
"Locust patch" doc lines, is the whole proposal. **To drop it:** when the pin moves to a Rune that has a sidebar
material and a way to read an element's material headless, delete `vendor/rune` and the `[patch]` table, switch
`window_glass.rs` to Rune's names, and keep the window tests in `src/ui/window_glass_tests.rs` passing. Moving the pin
while the patch is on means re-taking the copy from the new commit and re-applying the second commit's diff.

## Moving the pin

Move it on purpose, in its own commit, never alongside a feature.

1. Refresh the mirror, then read Rune's log from the old commit to the new one (`git log --oneline OLD..NEW` in the
   mirror). Note anything touching text input, IME, scrolling, images, `rune-kit` components, Markdown or Mermaid.
2. Change the `rev` in both lines of `Cargo.toml`, then `cargo update -p rune -p rune-kit`.
3. `cargo build --release` with no new warnings, and `cargo test --release` with every test passing.
4. Render the scenes headless and compare them with the last ones, at least `home`, `home-light`, `approval`,
   `approval-light`, `thread-rich-open`, `settings` and the composer scenes:
   `locust-native --snap out.png --scene <name> --size 1200x800 --scale 1`.
5. Use the window for a few minutes: type in the composer (including an accented character), scroll a long thread,
   open a menu, resize the window.
6. Write the old and new commit and what was checked in the commit message, and update the commit named above.
