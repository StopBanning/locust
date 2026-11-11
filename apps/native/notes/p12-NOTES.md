# P12 — Home, first launch, folders, board, away, what's new

P12 closes four Area 13 rows as **done** (folder registry / remembered folders; Home cover / team / starters; Board with Allow once / Deny; Since-you-were-away) and two as **partial** (first-launch boot + PATH discovery without install/sign-in UI; What's new / splash from native notes without self-update). Updates/self-update and tray/login item were out of round. Recount: **32 done, 34 partial, 87 missing, 153 total**. No window and no real turns were run.

## What changed

- `src/boot_view.rs`: Boot screen as a value (progress / summary / result words exact); PATH sweep via `runtime::discovery::find_named`.
- `src/folders.rs`: `folders.json` seed-once registry (list / use / learn / move / same_as / recover); Home Choose folder.
- `src/named_folder.rs`: message-named project checkout / worktree repository (lookbehind done by hand).
- `src/starters.rs`: Electron `STARTERS` / role starters / `TEAM_TEMPLATES` / `teammate_for_starter` / `fresh_starters_for`.
- `src/conversation_board.rs`: Needs you / Working / Ready to look at / Done columns; quiet Done-only → none.
- `src/away.rs`: `since_you_were_away` / `away_line` / attention mark on native `away.json` only.
- `src/changelog.rs`: CHANGELOG parse / groups / `<!-- big -->` / splash window; native notes constant (Electron's file never written).
- `src/ui/p12_panel.rs`: Home, Board (Allow once / Deny through `approval` / `native::decide_approval`), boot, away list, What's new Settings body + splash.
- Wiring: palette Home / Board; Escape; scenes `home`, `board`, `boot`, `away`, `whatsnew` (+ lights). `main.rs` wiring only.

Electron under `/Users/locust/work/locust/apps/desktop/src` was read only. Electron's data folder was not written.

## TypeScript behavior carried over

| Electron test source (under desktop/src) | Rust coverage |
| --- | --- |
| `shared/since-you-were-away.test.ts` | ran/failed/missed/waiting; awayLine; wasAway 20m; attention mark |
| `main/changelog.test.ts`, `main/whats-new-is-the-whole-changelog.test.ts` (parse/splash) | groups, big mark stripped, splash window, en-dash headings |
| `renderer/src/conversations-as-a-board.test.ts` | columns, order, rooms/trash out, needs-you / to-look-at |
| `main/every-folder-is-kept-by-its-path.test.ts`, `a-folder-is-one-folder-however-spelled.test.ts` (subset) | recover, use/list, same_as / same_folder_at_start |
| `main/a-message-that-names-its-project-starts-there.test.ts` (logic) | pathsNamedIn, checkoutOf, repositoryOf, projectNamedIn |
| `a-home-starter-starts-with-a-teammate-or-compares.test.ts`, `a-team-without-a-coder-gets-its-own-starters.test.tsx`, `a-build-starter-makes-one-page.test.ts` | STARTERS / role starters / page prompt |
| `main/a-team-template-makes-ordinary-teammates.test.ts` (titles) | TEAM_TEMPLATES + freshStartersFor |
| `the-boot-screen.test.ts` (progress/summary/words) | boot_view progress/summary/result/phase_at/homeRelative |
| Board Allow once / Deny words | BoardScreen exact; answers via approval channel |

## Remaining — not silently skipped

1. First launch: install / sign-in / Node / CRT animation / full DiscoveryEvent stream; PATH presence only.
2. Home: cover tube/swarm animation, Scout invite/watch, Compare / Start-from-issue execution (deferred words).
3. Board: Scout / routine wait cards (Keep/Discard); rooms still absent in native.
4. What's new: self-update / mac-self-update / linux-install out of round; splash uses native notes, not Electron CHANGELOG packaging.
5. Tray / login item / keep-running out of round.
6. Live window with Electron quit and `LOCUST_REAL_*` turns were not run.

## Verification, 10 November 2026

From `/Users/locust/Documents/Locust/native-review/p12/native` with:

```
PATH=$HOME/.cargo/bin:$PATH
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p12
```

- `cargo test --release` ×3 after last source edit: **488 passed, 0 failed, 13 ignored** each. Ignored stay the live MCP/runtime one-liners. Logs: `/Users/locust/work/scratch/p12-checks/test-final-{1,2,3}.log`.
- `cargo build --release`: exit 0, **zero compiler warnings**.
- `cargo clippy --release`: exit 0; four existing `too_many_arguments` in prior modules only. No new findings in P12 modules.
- `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py` against the release binary: exit 0 (synthetic). Logs: `/Users/locust/work/scratch/p12-checks/m1-cli.log`, `m3a-cli.log`.
- Scenes looked at under `/Users/locust/work/scratch/p12-snaps/` at `--size 900x900 --scale 1`: `home`, `home-light`, `board`, `board-light`, `boot`, `boot-light`, `away`, `away-light`, `whatsnew`, `whatsnew-light`. Inspected: Home claim / folder / team / starters; Board Needs you Allow once/Deny; boot PATH rows + summary; away Got it; What's new splash curly apostrophe.

Not verified: live window/exit, real turns, OS folder picker, install/sign-in, self-update.

## Review and fixes

Reviewed by Codec against the read-only Electron sources on 10 November 2026. Changes stayed in this review copy; no window or real agent turn was opened.

### Fixed, with regression coverage

- **Board approvals:** previously only `pending_approval` for the selected thread populated the Board. It now polls every waiting request from the native approval channel, keeps the oldest request per mission, and refreshes after decisions. Allow once and Deny still call `native::decide_approval` → `src/approval.rs`; no separate answer channel or automatic permission was added. Allow once observes the card's 500 ms fresh-click delay. A synthetic two-conversation test verifies no answer before a press, early approval refusal, allow/deny wire results, and exactly one recorded answer despite a repeated press. The test acquires the existing native test-hook lock, so saved-rule fixtures cannot interfere.
- **Board facts/counts:** use conversation rows before sidebar search/put-away filtering. The heading now shows both waiting and working counts, exactly `1 waiting on you · 1 working`. Running → completed/failed transitions while elsewhere enter Ready to look at; opening any member of the conversation clears its marks. Cancelled runs do not create a finish mark.
- **Away:** recap input now uses the same grouped conversation rows as the sidebar, preserving member/root ids instead of counting each continued turn as another run. Waiting includes Board requests outside the selected thread. Closing writes an attention mark to native data; a failed Got it write keeps the recap and reports the error. Run rows open their conversation, and failed/ran groups each have their own six-row allowance.
- **Folders:** lexical absolute paths preserve symlink spelling as Electron's `path.resolve` does; inode comparison remains separate. Windows folder names work on macOS too. Synthetic seed/use/move/learn/list tests assert Electron's `folders.json` bytes and directory contents remain unchanged.
- **What's new:** release headings and big markers now use Electron's exact regex shapes. Invalid semver/headings and `<!-- bigger -->` stay body text. Refresh clears an obsolete splash. The snapshot fixture now includes its synthetic previous version, so the splash contains release notes rather than an empty forced modal. Removed a boot timing debug line from the UI.
- **Test isolation:** `.cargo/config.toml` runs Cargo executables through `scripts/isolated-test-runner.py`. Before test startup, HOME, LOCUST_NATIVE_DATA, CODEX_HOME, XDG_CONFIG_HOME, APPDATA and TMPDIR point into one disposable directory; inherited MCP connection overrides are removed and LOCUST_REAL_* opt-ins fail. `src/test_isolation.rs` makes non-temporary Store roots/Electron paths and HOME-derived defaults panic before data access, with a rejection regression test. Existing runtime/Settings/usage/approval fixtures were moved into that temporary tree. The short temporary root also avoids macOS Unix-socket path limits. This is environment/Store-boundary enforcement, not a general interception of arbitrary filesystem syscalls.

### Electron test mapping and limits

- `renderer/src/a-request-is-answered-on-the-board.test.tsx`: native channel integration test above; real DOM bubbling, file-change/question navigation and live presses are not exercised.
- `renderer/src/conversations-as-a-board.test.ts`, `renderer/src/a-board-of-conversations.test.tsx`, `shared/since-you-were-away.test.ts`: existing pure reducer tests plus `board_and_away_match_electron_on_the_same_synthetic_rows`. `scripts/p12-home-oracle.mjs` executes the actual Electron `conversationBoard`, `conversationList`, `missionPhaseView` and `sinceYouWereAway` helpers against identical synthetic grouped rows, including root/member approval ids and trash. Existing away tests cover missed-slot deduplication, recorded/due cutoffs and hook-count exclusion.
- `main/every-folder-is-kept-by-its-path.test.ts`, `main/a-folder-is-one-folder-however-spelled.test.ts`, `main/a-message-that-names-its-project-starts-there.test.ts`: existing registry/named-folder tests plus the native-only write/symlink regression. Remote-origin capture and the full restored-folder workflow are not covered.
- `main/changelog.test.ts`, `main/whats-new-is-the-whole-changelog.test.ts`, `renderer/src/the-boot-screen.test.ts`: existing parser/boot tests plus malformed-heading and populated-splash regression checks. Boot animation and install/sign-in remain partial.
- Existing starter tests map to the Rust tests listed earlier. `renderer/src/an-empty-home-offers-a-team.test.tsx`, `home-leads-with-the-team.test.tsx`, `home-names-the-model-it-runs.test.tsx` and `home-free-model-keeps-its-tag.test.tsx` are **not fully ported**: Home currently renders names/roles, not the full interactive routed teammate cards.
- `the-home-screen-is-the-cover.test.tsx`, `home-fits-one-screen.test.tsx`, `home-fits-two-rows-of-teammates.test.ts`, `home-holds-still-when-tight-spacing-shrinks-it.test.ts`, `the-home-cover-rests-after-inactivity.test.ts`, and main's boot/home CSS, keyframe, narrow-home, spacing and scrolled-home tests remain **unported layout/animation behavior**; headless snapshots do not substitute for those tests.
- `a-scout-card-that-needs-you-is-on-the-board.test.tsx` and `a-routine-waiting-for-keep-is-on-the-board.test.tsx` remain outside the native feature set. Routine history is accepted by the pure away reducer but is not loaded into the Home recap yet; focus/minute attention updates are not wired. `since-you-were-away-on-home.test.tsx` therefore remains partial.

The earlier “done” summary must not be read as full Electron parity. Existing-folder selection still creates a native default project instead of opening an OS picker; first-launch/seen-version state is not persisted; startup boot/Home routing, rich Home teammate cards, quiet Done cards, routine/Scout integration, and full changelog Markdown rendering remain gaps. No claim of those behaviors passing is made here.

### Review verification

After the final source edit, using the requested PATH, LOCUST_TEST_TS_READER and CARGO_TARGET_DIR:

- `cargo test --release` three consecutive times: **496 passed, 0 failed, 13 ignored**, each. No compiler warnings. Logs: `work/review/test-verified-{1,2,3}.log`. The 13 ignored tests remain explicitly live-only; none was enabled or newly ignored.
- `cargo build --release`: exit 0, **zero warnings** (`work/review/build.log`).
- `cargo clippy --release`: exit 0, exactly the four documented existing `too_many_arguments` findings in approval/ledger_write/native; no new findings (`work/review/clippy.log`).
- Both `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py`: exit 0 against the final release binary, through the isolated runner (`work/review/{m1-cli,m3a-cli}.log`). Synthetic subprocesses only.
- Refreshed ten headless 900×900, scale-1 snapshots in **`work/review/snaps/` inside this review copy**, respecting the edit boundary; the earlier `/Users/locust/work/scratch/p12-snaps/` reference images were not overwritten. Inspected all ten surfaces; reinspected boot and What's new in both themes after correcting the debug line and empty splash fixture. Board shows both counts and Allow once/Deny, and the splash now contains release notes. These are static scenes, not live interaction tests.

Not run: any live window/exit, any real agent turn, OS folder selection, actual first-launch/install/sign-in, self-update, or dynamic layout/focus interactions. The initial guard failures and later test-hook interference were fixed before the three final runs. An attempted macOS sandbox wrapper was removed because it blocked existing `/bin/ps`/PTY tests; isolation uses the documented temporary environment and Store/default-path guards, not OS-wide filesystem sandboxing.
