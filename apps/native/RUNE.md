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
