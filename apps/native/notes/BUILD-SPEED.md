# Native test rebuild speed

Benchmarked on the 12-logical-CPU, 16 GiB arm64 Mac mini from `bc91d2af`, using Rust 1.100.0. The target directory was `~/work/scratch/native-target-build-speed-1`. It already held cached dependencies and artifacts from another worktree.

## Method

- For every build, ran `timeout 1800 cargo test -p locust-native --no-run --timings -v` from `apps/native`, with `CARGO_BUILD_JOBS=2`. Each run finished before the next started. The first build for each profile warmed its artifacts before edit timings.
- Alternated appending and removing a one-line comment in `src/ui/routines_panel.rs` and `core/src/path_title.rs`. Each measured run changed one file and used the preceding build's incremental cache. These timings measure rebuilding the filtered-test binary. An actual behavior change can invalidate more compiler work.
- Recorded command wall time, stable Cargo unit timings and the machine build gate's waits. Active wall time subtracts the observed parts of gate waits that did not overlap a measured local compiler. Gate waits are reported in full separately.
- Sampled compiler processes and their linker children about five times per second. The native pre-link interval includes Rust frontend and LLVM codegen work. Link time is the sampled Apple `ld` (or `ld64.lld`) process lifetime. Cargo's timing report also provides a frontend/codegen split for the core library when it reports the metadata handoff. For a core edit, the active wall includes rebuilding both crates.
- Sampled `sysctl vm.swapusage` about every five seconds. Rustc load is reported as sampled unique compiler PIDs and peak simultaneous processes, including other lanes. Very short processes can fall between samples.
- Baseline already passes `-C debuginfo=2 -C split-debuginfo=unpacked -C incremental=...` on macOS arm64, so `split-debuginfo = "unpacked"` and `incremental = true` are already the defaults and were not re-tested separately.

## Initial build

The initial command took 1,194 seconds wall time. Cargo compiled 10 units, including the stale Rune and Locust units. The gate recorded 66 seconds for Rune and 519 seconds for locust-core, with overlapping work. Cargo timed the native test unit at 477.5 seconds, including about 24.6 seconds in Apple `ld`. Swap use ranged from 6,633 to 10,017 MiB. The process sampler was corrected during this build after finding that macOS `ps` truncated executable names. Its final 268 seconds captured 122 compiler PIDs and a peak of five, and the edit trials have full sampling.

Apple's default `ld` reported project version 27106.1. Rust's toolchain also ships Mach-O `ld64.lld` 23.1.5. All trials except `lld` used Apple's linker.

## Summary by profile

Active time (`wall - non-overlapping gate waits`) is the primary metric; wall time is shown in parentheses. Medians are across 3 UI edits and 3 Core edits per profile (except `dev-line`, which had 2 UI and 2 Core edits from the first session).

| Profile | Setting | UI median active (wall) | Core median active (wall) | Median gate wait UI / Core | Median link UI / Core | Swap range (MiB) |
|---|---|---:|---:|---:|---:|---:|
| `baseline` | default (`debug = 2`, 256 CGUs, Apple `ld`) | 102.9 s (253.6 s) | 136.7 s (357.7 s) | 147 s / 221 s | 11.2 s / 8.3 s | 7,037–11,194 |
| `dev-line` *(n=2)* | `[profile.dev] debug = "line-tables-only"` | 47.5 s (145.0 s) | 159.9 s (278.4 s) | 98 s / 119 s | 5.2 s / 36.6 s | 6,811–10,566 |
| `test-line` | `[profile.test] debug = "line-tables-only"` | **72.3 s** (201.8 s) | **120.5 s** (170.6 s) | 64 s / 44 s | 7.0 s / 18.6 s | 5,922–11,712 |
| `cgu64` | `codegen-units = 64` on `locust-{native,core}` (`test`) | 98.9 s (259.9 s) | 213.3 s (337.7 s) | 17 s / 138 s | 6.6 s / 11.2 s | 6,690–11,039 |
| `test-line-cgu64` | `debug = "line-tables-only"` + `codegen-units = 64` on both crates (`test`) | 73.4 s (328.7 s) | 208.1 s (220.3 s) | 289 s / 24 s | 10.9 s / 8.6 s | 6,400–11,064 |
| `lld` | `RUSTFLAGS="-C link-arg=-fuse-ld=.../ld64.lld"` on baseline | 189.8 s (244.8 s) | 230.3 s (392.3 s) | 58 s / 193 s | 39.4 s / 63.0 s | 5,982–9,557 |

Notes on outliers:
- `cgu64` UI 2 had a 430 s Cargo pre-spawn delay before `rustc` started (active 492.5 s; `rustc` itself took 62.1 s).
- `test-line` UI 3 ran after switching back from `test-line-cgu64` without a re-warm, so `core/src/path_title.rs`'s updated mtime triggered a `locust_core` rebuild (135.8 s) during UI 3; `locust_native` itself took 62.6 s (55.6 s pre-link + 7.0 s link), consistent with UI 1 (63.8 s) and UI 2 (72.3 s).

## What each setting costs (full/warm build and disk)

| Profile | Warm build active (wall) | Units rebuilt on warm | Test binary size | Unpacked `.rcgu.o` per build (approx.) | Notes |
|---|---:|---:|---:|---:|---|
| `baseline` | 1,194.3 s (1,194.3 s) | 8 (stale workspace/Rune) | 312.7 MiB | ~1,172 MiB (256 CGUs) | Full DWARF (`debug = 2`) |
| `dev-line` | 967.6 s (1,488.3 s) | 327 (all deps + crates) | 300.3 MiB | ~599 MiB (256 CGUs) | Rebuilds every dependency and strips local-var DWARF from `dev` builds too |
| `test-line` | 2.3 s* (see note) | 0* (327 on cold `dev`) | 300.3 MiB | ~599 MiB (256 CGUs) | Reused `dev-line`'s cached deps in this target dir; unscoped `[profile.test]` rebuilds all deps on a baseline target dir and duplicates dep `.rlib`s |
| `cgu64` | 479.2 s (536.2 s) | 2 (`locust-core`, `locust-native`) | 312.1 MiB | ~918 MiB (64 CGUs) | Fewer, ~4× larger CGUs hurt incremental Core rebuilds (+77 s vs baseline) |
| `test-line-cgu64` | 372.7 s (641.7 s) | 2 (`locust-core`, `locust-native`) | 300.2 MiB | ~609 MiB (64 CGUs) | Zero dep rebuilds, but `codegen-units = 64` erased the Core win from line-tables |
| `lld` | 1,432.2 s (1,484.2 s) | 394 (all deps + crates) | 298.1 MiB | ~1,170 MiB (256 CGUs) | `RUSTFLAGS` invalidates all dep fingerprints; `ld64.lld` thrashed swap (24–102 s link vs Apple `ld` 5–19 s) |

### Why each rejected option lost

- **`codegen-units = 64` (`cgu64`, `test-line-cgu64`):** Incremental `dev`/`test` builds default to 256 CGUs. Cutting to 64 makes each CGU ~4× larger, so a one-line `locust-core` edit invalidates bigger CGUs and adds ~70–90 s of active compile time under memory pressure.
- **`ld64.lld` (`lld`):** On a 16 GiB Mac mini already using 6–10 GiB of swap across concurrent lanes, `ld64.lld` was 3–7× slower than Apple `ld` (median 39.4 s UI / 63.0 s Core vs 11.2 s / 8.3 s) and `RUSTFLAGS` forces a 394-unit full rebuild.
- **Unscoped `[profile.dev]` or `[profile.test]` `debug = "line-tables-only"`:** Rebuilds all 327 third-party dependency crates (~968 s active / ~1,488 s wall) and duplicates dependency `.rlib`s on disk (and `[profile.dev]` also strips local-variable debuginfo from normal `cargo run` builds).

## Per-run measurements

| Profile | Edit | Active (s) | Wall (s) | Gate waits (s) | Core rustc (s) | Native pre-link (s) | Linker (s) | Swap used (MiB) |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| `baseline` | UI 1 | 102.9 | 284.9 | 182 | — | 81.0 | 17.4 | 7,037–9,227 |
| `baseline` | UI 2 | 188.6 | 253.6 | 65 | — | 175.7 | 11.2 | 7,299–9,009 |
| `baseline` | UI 3 | 68.9 | 215.9 | 147 | — | 56.6 | 9.0 | 7,757–10,883 |
| `baseline` | Core 1 | 76.9 | 535.9 | 459 | 25.2 | 37.6 | 6.7 | 7,514–11,194 |
| `baseline` | Core 2 | 192.0 | 341.0 | 149 | 24.7 | 151.1 | 13.7 | 7,850–10,406 |
| `baseline` | Core 3 | 136.7 | 357.7 | 207 + 14 | 60.7 | 65.7 | 8.3 | 7,165–10,303 |
| `dev-line` | UI 1 | 65.0 | 176.0 | 111 | — | 33.9 | 5.2 | 7,883–10,566 |
| `dev-line` | UI 2 | 29.9 | 113.9 | 84 | — | 20.6 | 5.2 | 7,726–10,181 |
| `dev-line` | Core 1 | 181.7 | 202.7 | 21 | 35.7 | 77.0 | 66.1 | 7,500–10,141 |
| `dev-line` | Core 2 | 138.2 | 354.2 | 216 | 91.3 | 31.6 | 7.1 | 6,811–8,414 |
| `test-line` | UI 1 | 63.8 | 63.8 | 0 | — | 57.0 | 5.5 | 7,086–9,490 |
| `test-line` | UI 2 | 72.3 | 224.3 | 152 | — | 59.5 | 10.7 | 6,291–8,618 |
| `test-line` | UI 3 | 137.8 | 201.8 | 64 | 135.8 | 55.6 | 7.0 | 5,922–7,909 |
| `test-line` | Core 1 | 109.3 | 109.3 | 0 | 14.8 | 73.2 | 18.6 | 7,687–8,243 |
| `test-line` | Core 2 | 120.5 | 330.5 | 210 | 33.0 | 66.7 | 11.9 | 8,203–11,712 |
| `test-line` | Core 3 | 126.6 | 170.6 | 44 | 16.7 | 45.8 | 59.6 | 6,808–9,126 |
| `cgu64` | UI 1 | 45.7 | 62.7 | 17 | — | 37.0 | 6.6 | 7,767–8,925 |
| `cgu64` | UI 2 | 492.5 | 506.5 | 14 | — | 53.0 | 8.6 | 7,245–10,994 |
| `cgu64` | UI 3 | 98.9 | 259.9 | 161 | — | 79.9 | 6.3 | 8,086–9,961 |
| `cgu64` | Core 1 | 199.7 | 337.7 | 138 | 157.3 | 28.9 | 11.2 | 7,502–8,968 |
| `cgu64` | Core 2 | 213.3 | 213.3 | 0 | 26.3 | 154.3 | 29.9 | 7,741–8,822 |
| `cgu64` | Core 3 | 215.3 | 356.3 | 141 | 16.9 | 184.6 | 8.3 | 6,690–11,039 |
| `test-line-cgu64` | UI 1 | 39.7 | 328.7 | 289 | — | 26.9 | 10.9 | 8,025–10,170 |
| `test-line-cgu64` | UI 2 | 73.4 | 73.4 | 0 | — | 28.4 | 36.7 | 7,696–8,120 |
| `test-line-cgu64` | UI 3 | 156.6 | 470.6 | 314 | — | 138.5 | 6.8 | 6,400–11,064 |
| `test-line-cgu64` | Core 1 | 208.1 | 425.1 | 217 | 159.4 | 37.2 | 8.6 | 7,511–10,867 |
| `test-line-cgu64` | Core 2 | 217.3 | 220.3 | 3 | 15.8 | 190.4 | 8.1 | 7,207–9,050 |
| `test-line-cgu64` | Core 3 | 78.0 | 102.0 | 24 | 17.2 | 38.7 | 17.9 | 7,191–9,017 |
| `lld` | UI 1 | 250.5 | 824.5 | 574 | — | 145.8 | 101.5 | 6,608–9,557 |
| `lld` | UI 2 | 189.8 | 244.8 | 55 | — | 141.0 | 39.4 | 6,266–8,681 |
| `lld` | UI 3 | 100.0 | 158.0 | 58 | — | 65.8 | 28.6 | 6,711–7,708 |
| `lld` | Core 1 | 230.3 | 392.3 | 162 | 84.8 | 52.6 | 89.0 | 6,638–9,481 |
| `lld` | Core 2 | 121.3 | 314.3 | 64 + 129 | 67.9 | 22.3 | 23.8 | 6,037–7,483 |
| `lld` | Core 3 | 310.5 | 859.5 | 101 + 448 | 182.0 | 59.9 | 63.0 | 5,982–8,729 |

## Recommendation

Set `debug = "line-tables-only"` on our two workspace crates in the `test` profile only, keeping default `codegen-units` (256) and Apple's default `ld`:

```toml
[profile.test.package.locust-native]
debug = "line-tables-only"

[profile.test.package.locust-core]
debug = "line-tables-only"
```

Why this is the single best change:
- **Fastest balanced edit loop:** `test-line` cut UI edit median active time by **30%** (102.9 s → 72.3 s) and Core edit median active time by **12%** (136.7 s → 120.5 s), while every other tested change regressed Core edits.
- **Zero dependency rebuilds or duplicate dep cache:** Scoping to `locust-native` and `locust-core` (confirmed in `test-line-cgu64-warm`, which compiled only 2 units) reuses all 300+ cached dependency `.rlib`s from `dev`, avoiding the ~968 s / 327-unit cold rebuild and disk duplication of unscoped `[profile.dev]` or `[profile.test]`.
- **Half the unpacked object disk footprint:** Cuts `.rcgu.o` output per build from ~1,172 MiB to ~599 MiB and preserves full file:line panic backtraces in tests while leaving full DWARF intact for `dev` builds.
