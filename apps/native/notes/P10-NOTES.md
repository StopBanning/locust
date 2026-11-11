# P10 — Settings, Usage, Keyboard, Keychain

P10 closes two Area 11 rows as **done** (keyboard shortcut reference; keychain unlock note without storing secrets) and two as **partial** (categorized Settings; Usage/cost). Retention/backup, connectors, feedback, and GitHub account stay **missing**. Recount: **28 done, 29 partial, 96 missing, 153 total**. No window and no real turns were run.

## What changed

- `src/settings_pages.rs`: Electron `SETTINGS_PAGES` — 13 pages, five groups, headings, also-known-as search, exact chrome words (`Settings`, `Search settings`, empty-search sentence, `How it works`).
- `src/workspace_settings.rs`: `teammates.json` settings parse/save (seed-once via Store); `appearance.json` theme seed-once. Live: Theme, Sidebar layout, Swarm, Auto mode, memory mode, Between-teammates relay, native data folder path. Deferred pages/controls show Electron headings with `runs in Locust for now`.
- `src/usage.rs` + `src/spend.rs` + `src/usage_ledger.rs`: usage summary/meters/spend words; turns from native **and** Electron `mission-ledger` (Electron read-only); plan window `% left` parse without lookahead.
- `src/ui/usage_panel.rs`: Usage dialog — Activity / Limits / Models, range radios, spent sentence.
- `src/keyboard_shortcuts.rs` + `src/ui/keyboard_sheet.rs`: full Electron shortcut list; native-bound unmarked; lacking marked `runs in Locust for now`; Mac Cmd/Option remap with `controlOnMac`.
- `src/keychain_note.rs`: exact `KEYCHAIN_NOTE`; ask-gating only; **never** reads/copies keychain items. Banner in `main.rs` view when asking.
- Wiring: palette Settings / Usage / Keyboard shortcuts; Escape closes; scenes `settings`, `usage`, `keyboard`, `keychain-note` (+ lights). `Store::root()` made public for the data-folder line.

Electron under `/Users/locust/work/locust/apps/desktop/src` was read only. Electron's data folder was not written.

## TypeScript behavior carried over

| Electron test source (under desktop/src) | Rust coverage |
| --- | --- |
| `renderer/src/settings-pages-are-real.test.ts`, `settings-search-knows-the-words.test.ts`, `settings-read-like-claudes.test.ts` | page count/groups/ids; alsoKnownAs under real headings; memory/worktree/node/ledger search |
| `renderer/src/every-settings-switch-says-what-it-is.test.ts` (subset) | Swarm / Auto mode on/off Electron ledes |
| `renderer/src/ThemeSetting` words | Dark / Light / Match computer + theme ledes |
| `main/usage-is-counted.test.ts`, `renderer/src/usage-is-drawn.test.ts` (subset) | prompt tokens, summary ranges, paidBy/spentSentence, heat levels, compactCount reuse |
| `shared/spend.test.ts` (subset) | moneyOfUsage, dollars, month/limit refusal |
| `renderer/src/every-limit-says-what-is-left.test.tsx` | plan windows say `% left` |
| `renderer/src/keyboardShortcuts.test.ts`, `a-mac-says-cmd.test.tsx` | groups; keysOn Cmd/Option; controlOnMac keeps Ctrl |
| `main/the-keychain-is-asked-only-for-a-key-and-said-first.test.ts` (quiet paths) | no note without key; note words exact; no keychain IO |

## Remaining — not silently skipped

1. Settings: most General/Chat/Privacy/Agents/Cloud/Whats-new controls are shown deferred; project folder choose/check-after-edits UI; Look/Liquid glass; Home UsageLine chip; browser-held settings filter.
2. Usage: live send-time monthly budget refusal wiring; remembered usage-windows write path; heatmap month labels/columns as Electron draws them.
3. Keychain: encrypt/decrypt/safeStorage still absent (intentional — no secrets stored yet).
4. Retention/backup, connectors, feedback, GitHub account rows untouched.
5. Live window with Electron quit and `LOCUST_REAL_*` turns were not run.

## Verification, 10 November 2026

From `/Users/locust/Documents/Locust/native-review/p10/native` with:

```
PATH=$HOME/.cargo/bin:$PATH
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p10
```

- `cargo test --release` ×3 after last source edit: **424 passed, 0 failed, 13 ignored** each. Ignored stay the live MCP/runtime one-liners. Logs: `work/p10-checks/test-final-{1,2,3}.log`.
- `cargo build --release`: exit 0, **zero compiler warnings**.
- `cargo clippy --release`: exit 0; four existing `too_many_arguments` in `approval.rs` / `ledger_write.rs` / `native.rs` only. No new findings in P10 modules.
- `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py` against the release binary: exit 0 (synthetic). Logs: `work/p10-checks/m1-cli.log`, `m3a-cli.log`.
- Scenes looked at under `/Users/locust/work/scratch/p10-snaps/` at `--size 900x900 --scale 1`: `settings`, `settings-light`, `usage`, `usage-light`, `keyboard`, `keyboard-light`, `keychain-note`, `keychain-note-light`. Inspected: Appearance Theme/Sidebar live; deferred `runs in Locust for now`; Usage Activity/Limits `% left`/Models/paid-by; Keyboard Cmd remap + lacking marks; keychain amber Safe Storage / Always Allow banner.

Not verified: live window/exit, real turns, OS keychain prompt, Home Usage chip, send-time budget refusal end-to-end.

## Review and fixes

Codec review, 10 November 2026. The original “Live” list above overstated native behavior: Swarm, Auto mode, Sidebar layout, and Automatic replies only saved values. Their controls now say **runs in Locust for now**, as does the unimplemented Project folder chooser. Theme and memory mode remain actionable; Saved approvals opens the existing native panel. No runtime policy was expanded to make a toggle appear functional.

Fixed:

- Saved appearance is loaded during native startup, not only when opening Settings. Match computer's sentence uses the resolved palette. An empty Settings search result no longer falls back to an unrelated Appearance page.
- Usage preserves reported zero cache counts, so a measured 0% is not confused with an unreported cache. Receipt-less turns use their last recorded update; the first completion supplies the receipt, as Electron does. Duplicate mission IDs across both folders count once, with the native copy taking precedence.
- Copilot fractional premium requests survive display; counts use English grouping. Dollar ties match JavaScript `toFixed(2)` on the binary value (`0.125` → `$0.13`, `2.675` → `$2.67`). Null dollar aliases fall through like TypeScript's `??`. Exact model-sort ties preserve insertion order.
- Limits read Electron's actual `usage-readings.json`, choosing the newer timestamp across folders instead of overwriting native readings with stale Electron data. The previous filename, `usage-windows.json`, was incorrect.
- Added the dialog's “From the cache” statistic and corrected range/busiest-day words. Removed the fabricated monthly-cap footer: it treated the selected range as a calendar month and described caps that native does not enforce. Deferred budget helpers are test-only until there is a real caller.
- Snapshot inspection found long Mac keyboard chords overlapping their descriptions. Keys now wrap within their column, with a gap; deferred notes use the remaining width.

Regression coverage:

- `settings_panel::tests::every_settings_action_keeps_electron_fingerprint` fingerprints a synthetic Electron tree (paths, lengths, modification times and bytes), then opens Settings, cycles every theme and memory mode, builds every Settings page, opens Saved approvals' native panel and reads Usage; the fingerprint is unchanged. Theme/memory persistence is asserted. Existing `people` tests exercise memory-off and ask behavior.
- `usage_ledger` covers zero cache measurements, failed-turn date, duplicate native/Electron mission IDs, continuation roots, and newest remembered limit readings.
- `usage::tests::synthetic_receipts_match_actual_electron_helpers` requires Node and the reference sources; it does not silently skip. `scripts/p10-usage-oracle.mjs` strips TypeScript in memory and executes Electron's actual `shared/usage.ts`, `shared/spend.ts`, and `renderer/src/usageView.ts` on identical synthetic receipts for 7d/30d/all. It compares totals, pay, models, shares, cache text, days, heatmap data, streaks, active days, spending sentences and binary dollar rounding. It does not run Electron or write its sources/data.

TypeScript test accounting: `main/usage-is-counted.test.ts`, `shared/spend.test.ts`, `renderer/src/usage-is-drawn.test.tsx`, `settings-search-knows-the-words.test.ts`, `every-limit-says-what-is-left.test.tsx`, `keyboardShortcuts.test.ts`, `a-mac-says-cmd.test.tsx`, and `main/the-keychain-is-asked-only-for-a-key-and-said-first.test.ts` were reviewed with the named production modules. The original notes' `settings-pages-are-real.test.ts`, `settings-read-like-claudes.test.ts`, and `every-settings-switch-says-what-it-is.test.ts` are absent from the current reference tree; they were not claimed as executed. The source index and existing Rust search tests cover page names/groups/aliases. React markup, icons/keycaps, modal focus, Home sparkbars/chip, heatmap month-column rendering, and agent-account UI remain unported checks. Monthly limit integration and unpriced-own-model reporting remain deferred. Secret backend first-launch/encrypt/decrypt tests do not apply to native's note-only implementation; native has no OS keychain backend and setup passes `keeps_key=false`. No keychain API was added or invoked.

Remaining scope is still **Settings partial / Usage partial**. Twelve-week data is computed, but the native UI still shows its existing abbreviated recent-heat line; account refresh, persisted fresh usage readings, carried usage from another computer, full heatmap rendering, budget enforcement and Home usage are not completed here. Fractional token receipts, corrupt/oversized-ledger recovery and locale collation beyond the synthetic model names are not established as one-to-one parity by this review.

Final verification after the last source edit, using the requested PATH, `LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js`, and `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p10`:

- `cargo test --release` three times: **429 passed, 0 failed, 13 ignored** each. Logs: `work/p10-checks/review-complete-{1,2,3}.log`. Earlier failed review logs are retained; the final three supersede them.
- `cargo build --release`: exit 0, **zero compiler warnings** (`review-complete-build.log`).
- `cargo clippy --release`: exit 0, **no new findings**. Only the same four `too_many_arguments` findings in `approval.rs`, `ledger_write.rs`, and `native.rs` (`review-complete-clippy.log`).
- Both `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py` passed against the final release binary (`review-complete-m1-cli.log`, `review-complete-m3a-cli.log`).
- Rendered all eight requested dark/light scenes at 900×900, scale 1. All eight were inspected; the keyboard pair was rendered and inspected again after fixing overlap. Fresh files: `work/p10-review-snaps/`; render log: `review-complete-snaps.log`. The supplied `/Users/locust/work/scratch/p10-snaps/` reference was left unchanged to honor this turn's edit-only-this-checkout restriction. The dialog/sheet are scroll containers; snapshots establish their visible initial viewport, not live scrolling/focus behavior.
- Source review confirms no native OS keychain backend/calls; the synthetic Settings fingerprint test passed in all final runs. No Electron data writes were introduced. No production `unwrap()` or `allow(dead_code)` was added; previous production date-helper unwraps were removed.

The 13 intentionally unrun tests are live Electron team listing, real-ledger parsing, real Codex continuation, real Claude/Codex approval turns, and real one-line turns for ACP, Antigravity, Copilot, Cursor, Kilo, Muse, OpenCode and Claude. Live window/exit with Electron quit, OS appearance changes during a running window, actual scrolling/focus, and real runtime turns were **not run**. This review does not promote the partial Settings/Usage rows to done.
