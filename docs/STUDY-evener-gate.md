# Study: Evener's load-aware test gate, and what fits ours

Source read: `prime-radiant-inc/evener` at commit `2fa5463` (shallow clone, read only, nothing run). Our side:
`apps/native` at `2c912f2c`. Quoted Evener lines are marked; see section 5 on the licence.

## 1. What Evener does

**Worker sizing** (`scripts/lib/load-aware-workers.sh`, POSIX sh, sourced). `load_aware_workers CAP [CORES] [LOAD1]`
returns `CORES - ceil(LOAD1)`, clamped to `1..CAP`; `CAP <= 0` means "cores". The core count is `nproc`, else
`getconf _NPROCESSORS_ONLN`, then lowered to any cgroup CPU quota (v1 and v2, walking nested cgroups). Load is
`/proc/loadavg`, else `sysctl -n vm.loadavg` on macOS; `LOAD_AWARE_LOAD1` overrides it (their CI sets 0). An unreadable
load or core count falls back to CAP, never to a guess. The script itself says this is not admission control:

> From prime-radiant-inc/evener, MIT License, Copyright (c) 2025 Dan Shapiro:
> "It reads a 1-minute average once, at start, so runs that begin together still read the same load and can still
> stack. It narrows each run's claim; it does not serialize runs."

The test cases pin the formula: cap 4, 16 cores, load 12.0 gives 4; load 12.1 gives 3; load 43.27 gives 1
(`loadawareworkers_test.go`). `scripts/lib/gate-budgets.sh` wraps it as `gate_budget CAP DEFAULT` (an explicit env
value always wins; a missing helper or a 0 answer gives DEFAULT) and is used for `go test -p`, vitest
`--maxWorkers` (`scripts/native/run-tests.sh`) and shard concurrency (`scripts/gate/run-module-tests.sh`).

**Timing budgets** (`scripts/gate/test-timing-budget.sh`, verdict in `cmd/evener-dev/timingcompare.go`, budgets in
`testing-budget.json`). Per Go package they budget the test binary's **CPU time**, with wall time only as a backstop:
fail at 1.5x the CPU budget, warn at 1.1x, fail at 2.5x the wall budget. Fail limits are divided by the run's median
measured/budget ratio when above 1 (at least 3 packages with budgets of 5 s or more), so "a uniformly slow runner is not
a regression". Per test: tests named in `perTestCeiling` fail strictly over their own ceiling (15-40 s); every other
test is held to a flat 3 s and only warns, in every mode. Issue #4615 in the script header records them removing
whole classes from strict enforcement because their "wall time tracks how busy the runner is, not the code".
Strict (non-zero exit) only under `$CI` or `--strict`; locally the same lines print and exit 0.

**How a failure reads** (format strings from `timingcompare.go`):

    FAIL  <pkg>: 12.34s cpu over budget 5.00s (2.47x > 1.5x)
    FAIL  <pkg>: test <Name> took 41.20s, over its 30s ceiling (named '<Name>' in testing-budget.json perTestCeiling)
    WARN  <pkg>: test <Name> took 4.10s, over the 3.00s per-test ceiling (unnamed; warn-only, #4601)
    NOTE  this run's packages sit at a median 1.40x of their cpu budgets: the runner is slow, so cpu failure limits below are scaled by 1.40x

A malformed budget file or a package that ran with no CPU row is a `FAIL` in every mode, naming the file or package.
`timingbudget_test.go` drives all of this through a `--measured` fixture file, never a real `go test` run.

## 2. Which parts fit our gate

Our gate is `gentle-cargo.py nextest run --workspace`, then `gentle-cargo.py test --release --workspace`, on a 12-core,
16 GB Mac mini (`notes/BUILD-SPEED-NOTES.md`). `gentle-cargo.py` already does the admission control Evener lacks: a
file lock, two slots, the second only while load is under 8, `nice 10`, 4 build jobs. What it does not do is size
the test run itself: nextest defaults to `test-threads = "num-cpus"`, so every lane runs 12 tests at once whatever
the load. No `.config/nextest.toml` exists in `apps/native` today. (The brief says three lanes; `gentle-cargo.py`'s
docstring says two slots since 18 Nov.)

Mapped to nextest (keys checked against nextest's own docs source, `nextest-rs/nextest` at `3234fd5`, since
nexte.st was blocked here):

- **`test-threads`**: positive integer, negative integer (cores minus N), or `"num-cpus"`. The env var
  `NEXTEST_TEST_THREADS` overrides config, so a wrapper can compute Evener's formula and export it. Fits directly.
- **`slow-timeout = { period, terminate-after, grace-period }`**: marks SLOW after one period, kills the test's
  process group after `terminate-after` periods. Our process-waiting tests already give up after 60 s
  (`core/src/load_waits.rs` `UNDER_LOAD`, and the window pump in `src/ui/compare_window_tests.rs`). Set the kill above
  that so the test's own message wins, and use SLOW as our version of Evener's warn-only ceiling.
- **`[test-groups]` with `max-threads`**, assigned by `test-group` in an override: a semaphore for the headless window
  tests (`window_drive_tests`, 125 tests; `compare_window_tests`, 38), which are the heavy ones in `notes/LOAD-NOTES.md`.
- **`[[profile.<name>.overrides]]`** with `filter`: per-test `slow-timeout`, `threads-required`, `retries`. This is
  Evener's named `perTestCeiling` list in nextest form.
- **`retries`**: a retry that passes is reported FLAKY and, by default, the run still passes. That hides exactly the
  load failures we want to see. Keep `retries = 0` and allow it only for named tests, each with a reason and a plan to
  fix. `flaky-result = "fail"` exists (nextest 0.9.131+) if we want a flake to stay red.
- **Not a fit**: Evener's CPU-time-per-package budget file and median scaling. Worth it for a big Go monorepo with CI
  history; for us the cheaper lesson is the one in #4615: **don't assert wall time in a unit test.**
  `composer::file_mentions::at_file_tests::answers_a_bare_at_over_50000_paths_in_well_under_a_keystroke` asserts
  `< 250 ms` of `Instant` time; measuring the thread's CPU time (`clock_gettime(CLOCK_THREAD_CPUTIME_ID)`; `libc` is a
  dependency of `locust-core` only) would test the code, not the load. Inference, not tried.
- **The release step**: `cargo test --release` is libtest and ignores `nextest.toml`. Either the wrapper also sets
  `RUST_TEST_THREADS`, or the step becomes `cargo nextest run --release --profile gate` (then run `cargo test --doc`
  separately; nextest skips doctests).

The ACP test `runtime::acp::tests::acp_queue_overflow_fails_closed` (`core/src/runtime/acp_tests.rs`) has two fixed
waits: a 200 ms sleep before reading the flood and a 10 s deadline, not `UNDER_LOAD`. That is the likely load
sensitivity (inference; I saw no failure log for it). The fix belongs in the test; a named retry is a stopgap.

## 3. Proposed `apps/native/.config/nextest.toml` (not added)

```toml
# Proposed; untested. Workspace root is apps/native (members ".", "core").

[test-groups]
# Headless Rune windows: each spins a runtime and often a Python fake. Three at once per lane.
window = { max-threads = 3 }

[profile.default]
# SLOW at 60 s, killed at 180 s: above load_waits::UNDER_LOAD (60 s), so the test's own message comes first.
slow-timeout = { period = "60s", terminate-after = 3, grace-period = "5s" }
retries = 0

[[profile.default.overrides]]
filter = 'package(locust-native) & test(/^(window_drive_tests|compare_window_tests)::/)'
test-group = "window"

[[profile.default.overrides]]
# Asserts < 250 ms wall time. Run it alone in this run until it measures CPU time instead.
filter = 'package(locust-native) & test(=composer::file_mentions::at_file_tests::answers_a_bare_at_over_50000_paths_in_well_under_a_keystroke)'
threads-required = "num-test-threads"

[profile.gate]
# inherits "default". Thread count comes from NEXTEST_TEST_THREADS (set by the wrapper).
fail-fast = false
final-status-level = "slow"      # list SLOW tests at the end, like Evener's warn-only ceiling

# Named retries only. Each line needs a reason and an issue; remove it when the test is fixed.
[[profile.gate.overrides]]
# Fixed 200 ms sleep + 10 s deadline; should use load_waits::UNDER_LOAD.
filter = 'package(locust-core) & test(=runtime::acp::tests::acp_queue_overflow_fails_closed)'
retries = 1
```

Wrapper sketch, adapted from Evener's `load_aware_workers` (from prime-radiant-inc/evener, MIT License, Copyright
(c) 2025 Dan Shapiro):

```sh
#!/bin/sh
# apps/native/scripts/gate-tests.sh (sketch). Size the test run from spare CPU, then run both gate steps.
set -eu
cap=${GATE_TEST_CAP:-8}
cores=$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu)
load=$(sysctl -n vm.loadavg 2>/dev/null | tr -d '{}' | awk '{print $1}')   # Linux: cut -d' ' -f1 /proc/loadavg
case "$load" in ''|*[!0-9.]*) load= ;; esac
if [ -z "$load" ]; then workers=$cap; else      # unknown load is not an idle machine: keep the cap
  workers=$(awk -v c="$cores" -v l="$load" -v cap="$cap" 'BEGIN {
    w = int(l); if (l > w) w++                   # a partly busy core already costs a worker
    w = c - w; if (w < 1) w = 1; if (w > cap) w = cap; print w }')
fi
: "${NEXTEST_TEST_THREADS:=$workers}" "${RUST_TEST_THREADS:=$workers}"   # an explicit value wins
export NEXTEST_TEST_THREADS RUST_TEST_THREADS
echo "gate-tests: $NEXTEST_TEST_THREADS test threads (cores $cores, load ${load:-unknown}, cap $cap)" >&2
python3 scripts/gentle-cargo.py nextest run --workspace --profile gate
python3 scripts/gentle-cargo.py test --release --workspace
```

Better placement: do the same sizing inside `gentle-cargo.py` right after `_take_a_slot()`, with `os.cpu_count()` and
`os.getloadavg()` (both already used there). A wrapper reads the load before it waits for the lock, and the wait can be
minutes. Note load average measures CPU queueing, not memory; on 16 GB, memory may bind first (inference).

## 4. AppWire and device pairing: what Locust's phone work could take

Evener: `appwire/device_pairing.go` (types), `cmd/evener-hub/device_pairing.go` and `app_device_pairing.go` (logic and
the `POST /api/device-pairing` bootstrap), `docs/device-pairing.md` (wire contract), `docs/appwire-protocol.md`
(generated). Locust: `core/src/phone_host.rs` and `phone_server.rs` (Wi-Fi), `core/src/phone_anywhere.rs` and
`relay_crypto.rs` (relay), from `apps/desktop/src/main/phone-anywhere.ts` and `relay-crypto.ts`.

- **Copy: operator confirms a code bound to the device key.** Evener releases nothing until the operator picks the
  claim whose code matches the device: SHA-512 over `evener-pairing-v1` + invitation + device X25519 key, shown as 8
  groups of 4 hex on both sides; two claims with the same code refuse approval. Locust's relay pairing accepts the
  first device that presents a valid proof in its sealed hello (`phone_anywhere.rs`, `code.used = true`), with no
  comparison. So anyone who sees the QR within its 10 minutes can pair first, unnoticed (inference from the code).
- **Copy: retrieval is idempotent until expiry.** Evener: "Reading the result does not consume it: a lost HTTP reply
  must not strand an approved client." Locust clears the code after sending `paired`; if that frame is lost, the PC has
  saved a device the phone never got a token for (inference; I did not trace the phone page's retry).
- **Compare: bounds on the unauthenticated path.** Evener: 1024 bytes, 5 s read deadline, unknown fields refused,
  browser `Origin` refused, no CORS, at most 16 invitations and 8 claims each. Locust's `phone_server.rs` has its own
  caps (16 KiB head, 10 s/30 s timeouts, 32 open, chunked refused, key checked before the body); add a cap on
  concurrent claims per code if the comparison step is adopted.
- **Consider: one typed frame catalog.** AppWire's method list lives in one Go catalog that generates both the
  protocol doc and the TypeScript types, and CI checks both are current. Locust's relay frames (`pair-open`, `allow`,
  `send`, `pair-close`, `paired`) are `json!` literals in `phone_anywhere.rs`.
- **Don't copy:** Evener gives every device the same Hub token, with "no per-device revocation in this protocol".
  Locust already keeps per-device keys and can remove one device (`PhoneAnywhere::remove`).

## 5. What I could not verify

- **Licence holder.** Evener has no top-level `LICENSE` at `2fa5463`. The only licence file at the root is
  `LICENSE-kilroy` (MIT, "Copyright (c) 2025 Dan Shapiro"), which may cover a vendored part, not the whole repo; the
  GitHub API was not available here to read the repo's declared licence. The attribution above uses that file; confirm
  before copying any code.
- nexte.st was blocked; keys come from nextest's repository docs at `3234fd5`. I did not check which nextest version
  is installed on the Mac, so `flaky-result`, `final-status-level = "slow"` and `test(=...)` are unverified there.
- Nothing was built or run, on either side. The TOML, the filters and the wrapper are untested; the full test names were
  read from source (`src/main.rs` module paths, `core/src/runtime/acp.rs`), not from `cargo nextest list`.
- The ACP and wall-time diagnoses, the stale-load point, the memory point and both pairing gaps are inferences from
  code, not reproduced.
