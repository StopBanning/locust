# Build speed (16 Nov 2026)

Every test run and every break check used `cargo test --release`. Measured on the Mac mini (12 cores, 16 GB, four
cargo jobs at nice 10, other work running), after touching one file in `core/src`:

| Profile | Rebuild | Engine tests | Window tests | Edit to all tests passed |
|---|---|---|---|---|
| `--release` (before) | 146 s | 32 s | 61 s | about 4 min |
| debug, everything unoptimized | 17 s | 33 s | 140 s | about 3 min |
| debug, libraries at `opt-level = 2` | 20 s | 34 s | 71 s | 2 min 9 s |
| the same, run by cargo-nextest (now) | 20 s | 78 s, both at once | | **about 1 min 40 s** |

`[profile.dev.package."*"] opt-level = 2` in `Cargo.toml` builds Rune, regex, zip and the rest optimized once (4 min 19 s
cold), and our two crates in debug, so an edit rebuilds in about 20 s. A break check that runs one package pays the
rebuild once per break: about 20 s instead of about 146 s.

## The rule now
- Every day: `gentle-cargo.py nextest run --workspace --no-fail-fast`. It runs the engine's and the window's tests at
  once, each test in its own process with its own data folder. `gentle-cargo.py test --workspace` still works.
- Every break check: `gentle-cargo.py test -p <package>` (or the whole workspace). No `--release`.
- Once before a merge is pushed: `gentle-cargo.py test --release --workspace`, then `build --release`. The shipped app
  is the release build, and some behaviour differs between the two (below), so both must pass.
- `scripts/fixtures/renderer-mutation.py`, `~/work/scratch/mut6.py` and `~/work/scratch/maxlane/mutsafe.py` and
  `mutall.py` run debug now.

## What debug caught on its first run
Two pet tests compared `&CODEX_BUDDY` by address (`std::ptr::eq`) while `CODEX_BUDDY` was a `const`. A const has no
single address: each use is a copy. Release builds happened to merge the copies, so the tests passed there and failed
in debug. `CODEX_BUDDY` is a `static` now, which has one address, and the tests pass in both.

Debug also checks integer overflow, which release does not. That is a second reason to run it every day.

## Also changed
- `gentle-cargo.py` refuses to start when `LOCUST_TEST_TS_READER` names a file that is not there. On 16 Nov a path
  copied from another worktree failed 51 ledger tests with "TypeScript reader rejected", which looked like a real
  failure. Leave it unset: the default is the reader built in `~/work/locust`.
- `.github/workflows/native.yml` runs on `exec/native-engine` too. Before, it ran only on `exec/native`, so no
  engine push since 12 Nov had been checked on a clean checkout. The runner's guard admits `exec/native-engine` too
  (Colin's yes, 16 Nov), so the Mac test job runs there as well.

## cargo-nextest and cargo-mutants (installed 16 Nov, Colin's yes)
- nextest: 3,007 tests in 78 s against 105 s for `cargo test`. Its first run found one thing `cargo test` hid:
  `window_drive_tests::install_runs_the_rows_line_and_says_how_it_ended` passes, but leaves a child process still
  writing in its data folder after it ends, so the folder's removal failed. `isolated-test-runner.py` now ignores
  cleanup errors. Not yet fixed: the leftover process itself.
- cargo-mutants: the engine has 27,372 places it could break, far too many to sweep. Use it on what a batch changed:
  `cargo mutants -p locust-core --in-diff <(git diff <base>)` breaks only the changed lines, one at a time, and lists
  every break no test caught. It is the automatic version of our hand-picked breaks: run it beside them, not instead.
