# P9: Terminal and own commands

This is a partial parity implementation. No Area 7 row is closed. Current total: 26 done / 30 partial / 97 missing, 153 rows. Settings/usage was not changed; main.rs changes are module/state/view/poll/send/scene wiring only.

## Sources and implementation

Read PLAN.md, PARITY.md Area 7, team memory, Electron TerminalPanel/TerminalToggle, terminal-host, pseudo-terminal, own-command-host/IPC/shared helpers/OwnCommand, OpenInTerminal, open-in-terminal, mac-terminal, terminal-catch-up, and associated test cases. Electron source/data were not written.

- `src/terminal.rs`: portable-pty 0.9.0 for controlling terminal, kernel geometry and interactive child, rather than pipes masquerading as a terminal. vt100 0.18.1 (vte 0.15 underneath) for streaming UTF-8/ANSI state, alternate screen, colours, cursor and bounded 10,000-line scrollback. These are focused existing terminal libraries; Rune has no terminal emulator. API evidence: https://docs.rs/portable-pty/0.9.0/portable_pty/ and https://docs.rs/vt100/0.18.1/vt100/ .
- `src/ui/terminal_panel.rs`: at most eight tabs, default login shell, each new tab binds the selected teammate folder, hide retains shells, tab close/drop kills and reaps. Drag divider resizes PTY. Rune fixed-size monospace cell elements draw ANSI indexed/RGB foreground/background, bold/italic/inverse, wide-cell continuation and cursor. Page Up/Down accesses scrollback. Drag selects cells; Command-C copies selection or screen; paste uses bracketed mode when requested.
- `src/runtime/process.rs`: shared terminal descendant snapshot/kill before root kill, for foreground jobs that leave the shell's process group. Children remain unreaped until the owner polls or drops. No program-name killing.
- `src/own_command.rs`: only a leading `!`, stripped/trimmed command, max 16,000 UTF-16 units, selected folder, shared default login shell, inherited person environment. Direct typed commands never enter command_guard, agent approval, or saved approval rules. Missing/empty/too-long requests use Electron refusal strings. Stop/drop ends owned descendants and reaps shell. Stdout/stderr are merged and bounded; last 200 lines/32 KiB remain with omission notices, including flood truncation. UI entries are local, as Electron's commandEntries are; finished entries carry with the next message once using Electron's prompt wording. Output is persisted in the resulting mission prompt when that message is sent.
- `src/runtime_terminal.rs`: macOS `.command` script, exclusive create mode 0700, quoted folder/program/argv, self-removal and exec; validated per-runtime resume arguments. Opener is reaped, external terminal remains the person's. Claude/Codex transcript parsing excludes injected context, Claude print-mode/tool-result/sidechain turns and unfinished exchanges. Explicit Catch up terminal reads after newest chain end/watermark, writes native-only schema-27 continuation files atomically, and saves terminal-imports watermark. Own-model and active-run UI refusals exist.
- Terminal/own-command entry points enforce machine-wide terminal/agent/model/spend/highest-mode organization restrictions. The complete organization-policy parser/UI is not ported here.

## Rune capability and gaps

Confirmed in pinned Rune source: SystemMono TextStyle, fixed element geometry, text/background colours, font weight/italic, key/paste handlers, drag callbacks, layout reports, clipboard copy and scrolling primitives. Thus Rune can draw a styled cell grid and cursor, and host custom scrollback/selection/copy logic. Rune supplies no ready-made terminal widget, terminal IME/accessibility semantics, cell selection engine, terminal protocol responses, font cell metrics or terminal reflow contract.

Remaining panel gaps: fixed 7.2-point cell advance instead of measured font metrics; underlines/strikethrough, blinking cursor, device-status/cursor query responses, complete function/home/end/delete/application-cursor key encodings, mouse reporting, HTTP link hit testing, screen-reader semantics and full xterm behaviour. Input/paste/selection have synthetic rendering coverage but no live window/clipboard test. Startup is synchronous rather than Electron's asynchronous late-spawn/disposal contract. Screen updates use bounded shared parser state, not Electron's explicit 16 ms pause/resume queue. Selected folder currently follows teammate configuration rather than resolving every conversation-owned worktree. The 900-pixel scene leaves the conversation/composer cramped; screenshots expose this rather than hiding it.

Remaining own-command gaps: project/global Markdown slash templates and folder allowances are outside the requested typed `!cmd` path and remain unimplemented. Full command-mode/stop accessibility, status positioning, space-only command wrapping guarantees, background-agent shell rendering and complete TS assertion coverage are not claimed. Carry state is marked at dispatch; failed-send carry rollback still needs coverage. Commands have a process elapsed label, whereas Electron's own-command component itself has only status; the requested duration test refers to agent activity rows.

Remaining runtime-terminal gaps: explicit catch-up rather than focus/pre-send integration, transcript stat cache, imported-turn ownership reassignment/TerminalDivider, account-specific launch environments/prefix arguments, automatic newest-tip refresh after import, cross-window serialization, exact offer/menu/tooltips and cloud/attach/setup workflows. Non-Claude/Codex sessions must not claim catch-up support. No external Terminal was opened. Imports have a focused native-reader/atomicity test, not the entire Electron catch-up integration matrix or a new TS reader test for imported records. UTF-16 import truncation edge cases still need coverage.

## TypeScript test disposition

“Partial” means applicable assertions remain unported, not silently passed or deemed inapplicable. Rust tests live in terminal.rs, own_command.rs, runtime_terminal.rs and ui/terminal_panel.rs. Existing native activity tests cover durations/background endings but do not establish full parity with every assertion below.

| Electron test under desktop/src | Disposition |
| --- | --- |
| shared/a-command-output-keeps-its-tail.test.ts | Partial: Rust output_tail_and_unicode covers last lines/bytes/Unicode; exact omitted-byte matrix remains. |
| shared/a-command-reaches-the-next-message-once.test.ts | Partial: own_command_carries_once_and_stays_in_its_conversation covers carry once, no pending carry, wrong conversation; stopped/running formatting matrix remains. |
| shared/a-command-file-becomes-a-prompt.test.ts | Leading bang/escape covered; Markdown templates/frontmatter/positional arguments and slash choice are outside typed-command scope; one-shot Windows/WSL variants do not apply to this macOS pass. |
| main/a-bang-runs-in-the-conversations-folder.test.ts | Partial: real synthetic command/default shell path and result; process-tree stop/policy implementation lacks matching full fake-host tests. Global/project template cases outside typed-command scope. |
| main/a-bang-refuses-a-folder-from-the-window.test.ts | Electron IPC/window ownership checks do not apply: native has no IPC endpoint accepting folders/shells; only State-selected context reaches entry points. Markdown folder-opening tests outside scope. |
| main/a-terminal-belongs-to-the-person-and-ends-with-the-app.test.ts | Partial: PTY printf output/ANSI/resize/exit; process-tree disposal implemented but fake lifecycle, policy, backpressure, unknown-worktree and late-spawn assertions remain. |
| main/a-terminal-refuses-paths-and-requests-from-other-windows.test.ts | Electron IPC ownership/payload shape does not apply to in-process native handlers. Native path/refusal tests remain incomplete. |
| renderer/src/the-terminal-opens-before-the-first-message.test.tsx | Partial: toggle before ledger exists; shortcut, accessibility and IPC chunk-boundary tests remain (native bytes stream directly to parser). |
| renderer/src/the-header-is-a-row-of-icons.test.tsx | Applicable terminal icon/hover policy assertions unported; current control is text in status row. Browser portions outside Area 7. |
| renderer/src/a-bang-in-the-box-is-command-mode.test.tsx | Partial: Escape strips mark; mode note, result/Stop draw; accessibility/static markup assertions remain. Project template prompt outside typed scope. |
| renderer/src/a-command-says-how-long-it-took.test.ts | Agent activity duration tests, not OwnCommand; existing ledger/UI duration tests retained. Complete assertion-by-assertion comparison remains. |
| renderer/src/a-background-command-says-what-became-of-it.test.tsx | Agent activity, not own commands; existing helper_background_end_updates_original_row and activity tests retained. Full static-render parity remains. |
| renderer/src/a-command-wraps-at-its-spaces.test.tsx | Applicable approval/Board command typography not newly ported; own text uses Rune wrapping; no claim that every flag wrap assertion is met. |
| main/a-conversation-opens-in-its-own-terminal.test.ts | Partial: resume token rules and flags; newest-session lookup and busy/own-model refusals implemented. Windows Terminal/cmd cases not applicable on macOS. Remaining host facts/launch matrix unported. |
| main/a-mac-opens-its-own-terminal.test.ts | resume_tokens_and_mac_shell_quoting covers script folder, self-removal, quoting, exec; environment/account variants unported. |
| main/what-happens-in-the-terminal-comes-back.test.ts | Partial: Claude cli-only/completion and Codex injection/completion/after cutoff; native import readability/atomicity. Dedup, deleted imports, all branch/running/owner/cache/long-prompt/race assertions remain. |
| renderer/src/a-conversation-offers-its-terminal.test.tsx | Offer/menu/accessibility/per-runtime words not fully ported; partial row retained. |
| renderer/src/the-terminal-turns-are-drawn-as-yours.test.tsx | TerminalDivider/ownership rendering unported. |
| main/the-terminal-lists-wsl-by-distribution.test.ts | Windows registry/WSL not applicable to macOS pass. |
| main/a-linux-terminal-is-the-one-the-desktop-has.test.ts, main/a-linux-app-sees-the-terminals-path.test.ts | Linux launcher/environment not applicable to macOS pass. |
| main/a-mac-app-sees-the-terminals-path.test.ts | Broader app environment acquisition not changed here; unported. |
| main/a-cloud-session-continues-in-its-own-terminal.test.ts, renderer/src/a-cloud-terminal-says-what-locust-can-see.test.tsx | Cloud worktree/teleport workflow outside requested local terminal scope. |
| main/past-opencode-and-cursor-chats-can-be-imported.test.ts, main/a-conversation-can-be-imported.test.ts | Standalone import workflows outside Area 7 catch-up. |
| main/an-added-account-has-only-its-own-home.test.ts | Account management outside scope; recorded-account terminal launch assertion remains a gap here. |
| renderer/src/the-window-in-a-browser-says-what-stays.test.tsx, renderer/src/a-control-says-it-works-on-the-computer.test.tsx | Browser-host refusal/remote controls not applicable to native in-process window. Native policy presentation remains a gap. |

## Validation

Final commands ran from this folder with PATH=$HOME/.cargo/bin:$PATH, LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js and CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p9. No live window, real agent turn or external Terminal was launched. PTY subprocess test uses only `/bin/sh -c 'printf ...'`. The scene parser uses synthetic bytes and starts no shell. Earlier exploratory suites included the now-fixed NUL fixture failure and one approval timeout during overlapping runs; these are not counted as final passes.


- Three sequential final `cargo test --release` runs: **410 passed, 0 failed, 13 existing live tests ignored** each (15.62 s, 16.70 s, 14.50 s). No new ignored tests. Existing adapter/mission TypeScript-reader compatibility checks ran with the configured reader.
- Final `cargo build --release`: pass, **zero warnings**.
- Final `cargo clippy --release`: pass; **four existing too_many_arguments findings** in approval.rs, ledger_write.rs and native.rs. No findings in the new terminal/command modules and no new suppressions.
- `python3 scripts/test-m1-cli.py <final binary>` and `python3 scripts/test-m3a-cli.py <final binary>`: pass, all synthetic scenarios, including cancellation/flood/approval/permission bridge.
- Final dark/light p9-terminal scenes: `/Users/locust/work/scratch/p9-snaps/terminal-dark.png` and `terminal-light.png`, `--size 900x900 --scale 1`, rendered headlessly and inspected. No window opened.
- Parity status-cell recount independently confirmed **26 done / 30 partial / 97 missing = 153**. Area 7 remains three partial rows. This does not establish the unported TS assertions or live PTY/clipboard/external Terminal behavior listed above.
- Logs are in `work/p9-validation/`. Only tests use unwrap; no production unwrap or allow(dead_code) was introduced.

## Review and fixes

Parity pass against Electron (`TerminalPanel.tsx`, `TerminalToggle.tsx`, `main/terminal-host.ts`, `main/pseudo-terminal.ts`, `OwnCommand.tsx`, `main/own-command-host.ts`, `own-command-ipc.ts`, `shared/own-commands.ts`, `OpenInTerminal.tsx`, `main/open-in-terminal.ts`, `main/mac-terminal.ts`, `main/terminal-catch-up.ts`, `main/terminal-folder.ts`, and the bang/terminal tests beside them). Fixes landed in this copy only. No PARITY row was promoted to done.

### Own-command recorded output matches Electron’s omission note

- Flood truncation previously appended a second `"Left out N KB…"` sentence (and could print `0 KB`). `own_command::cap` now returns `(text, lines, bytes)`; `omitted_note` matches Electron `omittedNote` (lines and/or bytes/KB with `max(1, round(kb))`); `poll` merges stream-dropped bytes into that one note (`omittedNote(lines, cappedBytes + dropped)`).
- Covered by `flood_bytes_merge_into_one_omitted_note` and updated `output_tail_and_unicode`.

### Own commands / terminals use the engine’s folder resolution

- `terminal_panel::folder` used only `teammate.folder`. It now calls public `native::folder_for` (teammate folder → Electron `workspace.json` → `~/Documents/Locust`), the same host path engine turns use.
- Conversation-owned worktree paths (Electron `terminalFolder` when `worktree: true`) remain unresolved; still listed under Remaining panel gaps.

### PTY / own-command kill and reap

- `end_terminal_tree` matched Electron’s kill order incorrectly: every descendant went through `kill_process` (`kill(-pid)` then `kill(pid)`). Descendants are now signaled by **exact PID only**; only the shell root gets the process-group kill (plus the individual fallback). `descendants_of` mirrors Electron’s BFS-then-reverse order.
- `Running::stop` still ends the process tree, and also `kill`s the shell directly when the tree release misses it (Electron’s release-then-`kill` fallback).
- `on_close` calls `terminal_panel::dispose`, which clears tabs and bang commands so PTYs are ended when the app closes (Electron `dispose`), not only when `State` is later dropped.
- Tests: `descendants_of_matches_electron_order_and_skips_unrelated`, `end_terminal_tree_kills_descendants_and_reaps_root`, `drop_kills_and_reaps_pty_shell`, `stop_kills_and_records_stopped_without_exit`, `dispose_clears_tabs_and_commands`.

### Shell quoting and hostile ANSI

- `resume_tokens_and_mac_shell_quoting` extended with metacharacters / `$(…)` / `;` paths so quoted `.command` script words cannot become shell words.
- `ansi_hostile_sequences_do_not_panic` feeds incomplete OSC/CSI, RGB/indexed colour, alternate screen, bracketed paste, NULs, invalid UTF-8, and a long CSI flood through `vt100` without panicking.

### Secrets in recorded bang output

- Electron `promptWithCommands` / native `terminal_panel::carry` both put raw command output into the next mission prompt; neither scrubs at carry time. Display/ledger scrubbing stays on the existing `redact_secrets` path. No change.

### Left unchanged (still partial)

- Markdown/slash own-commands, project allow-once (`command-allows.json`), full worktree cwd, 16 ms pause/resume queue, late-spawn dispose, measured cell metrics, mouse reporting, TerminalDivider/ownership rendering, live window/clipboard/external Terminal, and the unported TS assertion matrix in the table above.

### Verification this review

```sh
PATH=$HOME/.cargo/bin:$PATH \
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p9 cargo test --release
# ×3 after last source edit → each: 417 passed; 0 failed; 13 ignored

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p9 cargo build --release
# zero compiler warnings

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p9 cargo clippy --release --all-targets
# no new findings in own_command/terminal/runtime_terminal/terminal_panel; existing items_after_test_module / native_window_tests / too_many_arguments noise only

python3 scripts/test-m1-cli.py $HOME/work/scratch/native-target-p9/release/locust-native
python3 scripts/test-m3a-cli.py $HOME/work/scratch/native-target-p9/release/locust-native
# synthetic CLI scenarios pass, including cancellation/flood/approval/permission bridge
```

- Dark/light p9-terminal scenes re-snapped and inspected at 900×900, scale 1 under `/Users/locust/work/scratch/p9-snaps/` (`--snap`, not a live window): own-command `!printf hello` / Exit 0 and terminal grid `hello` in green are visible; conversation/composer remain cramped at 900 px as noted above.

Not verified: live window with Electron quit, real runtime turns (`LOCUST_REAL_*`), external Terminal open, worktree cwd, Markdown/slash own-commands, and the rest of the TypeScript suites listed under Remaining work / the disposition table.
