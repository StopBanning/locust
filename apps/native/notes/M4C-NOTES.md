# M4C — Cursor, Copilot, Antigravity, Muse

M4C ports the last four named runtimes onto the native Runtime trait in this copy only (`/Users/locust/Documents/Locust/native-review/m4c/native`). TypeScript under `/Users/locust/work/locust/packages/runtime-adapters` was read only. No credential files were read or copied, no real turn ran, and no window was opened.

## Per-file port map

| Rust file | TypeScript source | Scope |
| --- | --- | --- |
| `src/runtime/cursor.rs`, `cursor_events.rs`, `cursor_tests.rs`, `scripts/fixtures/m4-cursor.py`, `scripts/fixtures/cursor/` | `cursor-events.ts`; `commands.ts` cursor-agent launch (prompt on stdin; **no model flag** unless TS passes one — it does not) | stream-json → ledger; Ask/Edit/Auto sandbox flags; resume; Stop; synthetic + ignored `LOCUST_REAL_CURSOR=1` |
| `src/runtime/copilot.rs`, `copilot_events.rs`, `copilot_tests.rs`, `scripts/fixtures/m4-copilot.py`, `scripts/fixtures/copilot/` | `copilot-events.ts`; `commands.ts` Copilot `-p` launch (stdin none) | tool/message/stderr policy denial; session id; synthetic + ignored `LOCUST_REAL_COPILOT=1` |
| `src/runtime/antigravity.rs`, `agy_events.rs`, `antigravity_events.rs`, `antigravity_projects.rs`, `antigravity_tests.rs`, `scripts/fixtures/m4-agy.py`, `scripts/fixtures/agy/`, `scripts/fixtures/antigravity/` | `agy-events.ts`, `antigravity-events.ts`, `antigravity-projects.ts`; `commands.ts` / `createAgyPrintCommand` (`agy`, stream-json in/out, `-p=`, model/effort/conversation flags as TS) | Primary driver is the **agy CLI**; transcript normalizer + projects protobuf helpers are secondary. Synthetic + ignored `LOCUST_REAL_ANTIGRAVITY=1` |
| `src/runtime/muse.rs`, `muse_events.rs`, `muse_tests.rs`, `scripts/fixtures/m4-muse.py`, `scripts/fixtures/muse/` | `muse-events.ts`; `the-prompt-muse-cannot-be-told` / `createMuseExecCommand` (`--prompt-file` placeholder, never prompt in argv) | Envelope JSONL; internal vs person tasks; billing/sandbox/quota sentences; synthetic + ignored `LOCUST_REAL_MUSE=1` |
| `src/runtime/mod.rs`, `src/native.rs`, `src/run_cli.rs`, `src/ledger_write.rs`, `src/main.rs` | Dispatch / supported / resume / CLI labels | `native::supported()` includes `cursor`, `copilot`, `antigravity`, `muse`; `--run` accepts each; approval card labels |

## Tests ported and skipped

**Ported (fixture / synthetic):** Cursor `cursor-events.test.ts` core fixtures (read-only, write/shell rejected, resume, plan, usage, unknown/reconnect); Copilot `copilot-events.test.ts` core fixtures (plain reply, apply-patch, tools denied, stderr policy); Antigravity `agy-events.test.ts` core fixtures (reads-a-file, answered-then-unavailable, write-denied, edit-then-command-denied, quota, conversation-not-found, unknown-model, print args / user line); Muse `muse-events.test.ts` echo capture + person-tool / sandbox-once / billing / cancelled / session-id facts; Muse prompt-file unit tests from `the-prompt-muse-cannot-be-told.test.ts`.

**Skipped (and why):**
- `antigravity-waiting-is-said-as-waiting.test.ts`, `antigravity-question.test.ts`, `antigravity-gaps.test.ts`, `antigravity-background.test.ts` — host/UI waiting and question cards, not the CLI stream normalizer.
- Full `antigravity-events.test.ts` transcript suite — transcript path is ported as `antigravity_events.rs` + fixtures under `scripts/fixtures/antigravity/`; live driver is agy, so only projects unit tests + fixture presence; not every transcript assertion.
- `parseAgyModelList` cases from `agy-events.test.ts` — model-list parsing not required for the print/stream driver in this slice.
- Synthetic “stream interrupted while tool ACTIVE” / “sentence before step” inline cases from `agy-events.test.ts` — covered in spirit by answered-then-unavailable + edit fixtures; exact inline scripts not duplicated.
- `a-real-muse-wonder-run-reads-whole.test.ts` / wonder fixture walk — paid-provider capture; echo fixture is the measured free source of truth.
- `a-muse-plan-used-up-is-said-once.test.ts` — quota sentence helpers exist; dedicated plan-exhaust fixture not re-run as a separate file.

Each runtime has a fake process ledger accepted by `LOCUST_TEST_TS_READER` and an `#[ignore]` real one-line test gated on `LOCUST_REAL_<RUNTIME>=1` in a temp folder with a fake Electron folder.

## Verification

Commands from this folder:

```sh
PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m4c \
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js \
cargo test --release

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m4c \
cargo build --release

python3 scripts/test-m1-cli.py $HOME/work/scratch/native-target-m4c/release/locust-native
python3 scripts/test-m3a-cli.py $HOME/work/scratch/native-target-m4c/release/locust-native
```

- `cargo test --release` (with TS reader): **271 passed, 0 failed, 13 ignored**, three consecutive runs.
- `cargo build --release`: passed.
- `scripts/test-m1-cli.py` / `scripts/test-m3a-cli.py`: passed (synthetic Codex/Claude scenarios only).
- TypeScript reader accepted Cursor / Copilot / Antigravity / Muse synthetic ledgers.

PARITY Section 1 recounted from status cells: **153 rows — 21 done, 13 partial, 119 missing**. Composite named-runtime row stays **partial**.

## Unconfirmed

- Live `cursor-agent` / `copilot` / `agy` / `muse` one-line turns (`LOCUST_REAL_*=1`).
- Window exit while a native turn is running.
- Windows launch shims and process-tree kill for these four CLIs.
