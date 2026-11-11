# P5 — partial parity implementation

This round is **not complete**. It adds reply/code Copy requests, file-reader controls and line targeting, measured context usage and compaction lines, interrupted-record information, and the requested palette backdrop. It does not implement Compact now, reconciled checkpoint resume, or all Electron copy semantics. No row is promoted to done. Section 1 and Section 3 of PARITY.md were recounted: **153 rows: 25 done, 18 partial, 110 missing**.

## Source and boundaries

Read PLAN.md, PARITY.md section 4 round 5, and the requested renderer sources under `/Users/locust/work/locust/apps/desktop/src/renderer/src`. Those files and Electron's data were not modified. All implementation is in this native copy. The executable and scene files use the explicitly requested external scratch paths. No window or real turn was launched.

## Implementation

- `src/ui/reply_actions.rs`: Rune Markdown code-frame hooks retain the normal parser, lists, quotes and fences. Syntax colouring is suppressed while streaming and above 60,000 UTF-16 units. Code and reply Copy request the OS clipboard via Rune. File links in Markdown open the native reader; inline file references also have explicit controls. File targets retain `:12-18` and `#L12-L18` ranges. Files are bounded to 1 MiB of UTF-8 and canonical paths must stay inside the conversation folder. Reader opens near the range, highlights it, offers whole-file Copy and Close, and never launches files.
- `src/ui/context_usage.rs` and `src/ledger.rs`: only runtime-reported contextTokens/contextWindow produce a reading. The pie fills clockwise from twelve, goes amber at 80%, and uses Electron's context sentence. Info-level `*.context_compacted` events become a plain diagnostic line, invalidate previous readings, and remain separate from replies. Later measured usage restores the reading; session ancestry carries the newest valid reading.
- `src/ui/recovery.rs` and `src/ledger.rs`: keeps bounded checkpoint information; selects the newest epoch; distinguishes absent/unsafe checkpoints, integrity problems, unknown outcomes and shutdown cancellation from intentional Stop/completed/failed runs. Cards retain Electron's title and refusal words and name unverified actions. **There is no Resume button**: native has no reconciled checkpoint execution operation. The card explicitly says this instead of sending an ordinary follow-up or retrying an action.
- `src/main.rs`: command palette is a modal layer over the existing sidebar/thread, with a dimmed backdrop. It no longer replaces the app with an empty window.

## TypeScript test disposition

Ported means the named Rust tests cover the behavior described below, not that the TS test runner ran. Twelve Rust tests were added. Remaining TS assertions are explicitly deferred and keep composite rows partial.

| TS test file | Rust coverage or explicit gap |
| --- | --- |
| `code-in-a-reply-is-coloured.test.tsx` | `reply_actions::tests::code_preserves_unicode_and_has_bounded_colouring` checks real Rune highlighting, valid UTF-8 boundaries and unknown-language fallback. The streaming/60k guards are implemented. Full alias/file-name matrix, all Shiki grammars, token-theme assertions, cache warming and React effect lifecycle assertions are not ported: Rune uses its own synchronous scanner/cache and has fewer grammars. This is a parity gap for the missing languages. |
| `a-change-can-be-copied.test.tsx` | All four tests deferred: diff after-text reconstruction, deleted/new-file semantics, changed-file rows and opened-change Copy require the Changes/diff implementation, which remains missing. Reply Copy does not satisfy these tests. |
| `a-change-is-coloured.test.tsx` | All four tests deferred: diff-side lexical context and changed-word overlays are not implemented by this round. |
| `the-file-viewer-renders-and-never-runs.test.tsx` | `files_are_read_only_and_confined` checks source reading and outside-folder refusal. Native text elements never interpret source or launch files. The visible no-launch statement is inspected in both reader scenes. Markdown prose mode, version strips/reconstruction, Reveal, Save a copy, and activity-file row controls remain unported; no DOM-markup assertions are claimed as Rust tests. |
| `a-file-opens-on-itself.test.ts` | The native reader replaces its entire target/result when opening a file and has no selected historical version. Version/diff fold reset assertions do not apply until history is implemented; no equivalent source-string assertion was added. |
| `a-compaction-is-a-line-not-a-reply.test.ts` | `context_usage::ledger_tests::compaction_is_a_line_and_invalidates_context` proves an info-level compaction is a Note, not an Assistant message. The existing OpenCode normalizer suppresses synthetic continuation/summary text, but the exact end-to-end TS fixture was not duplicated or newly tested here. |
| `a-full-context-is-compacted-from-the-ring.test.tsx` | Compaction invalidation and restored later usage are covered by the ledger test. All offer/menu/queue/route/startup-button assertions remain unported. Native command discovery currently passes an empty runtime-command list, and sending `/compact` as an ordinary prompt is not a substitute for runtime compaction. |
| `the-ring-reads-what-the-conversation-holds.test.ts` | `only_runtime_measured_context_is_shown` checks the runtime denominator, absent/zero usage, percentage cap and exact 164k/200k sentence. Cost/subscription totals and exact SVG path assertions are deferred: cost is not shown and Rune draws a native path rather than SVG. Million-token formatting is implemented. |
| `the-context-menu-shows-what-fills-it.test.tsx` | Both category-menu assertions deferred: context breakdown/menu is not implemented. |
| `resume.test.ts` | Seven `recovery::tests` cover all decision cases: newest/absent checkpoint, safe record, named doubt, unsafe/integrity refusal, completed/failed/manual/no-checkpoint cancellation suppression, and shutdown cancellation. Several TS cases are combined within Rust tests. Actual checkpoint execution remains missing. |
| `a-reload-keeps-every-comparison-column-live.test.ts` | All six tests deferred. Native does not have Electron's renderer reload, host-owned comparison columns, or pending React IPC update map. Native retains process handles and tails ledgers; no claim is made that this implements host-process reattachment or comparison recovery. |

No dedicated `copyText.test`, `CopyButton.test`, `ColouredCode.test`, `FileViewer.test`, `ContextRing.test`, `ResumeCard.test`, or `recoverLiveRuns.test` exists beside those sources. Related integration tests above were used instead. Browser clipboard/textarea fallback does not apply to Rune. **Copied/Not copied feedback** was added in Review and fixes (confirm via `read_clipboard` after `Tx::copy`); live OS pasteboard contents were still not tested.

Other tests importing FileViewer for image zoom, spreadsheet grids, browser call refusal, browser-only controls or runnable page previews belong to the separate image/sheet/page/browser surfaces. Those surfaces were not changed or credited here.

## Verification

- Requested release suite, with `LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js` and `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p5`: **321 passed, 0 failed, 13 ignored**, three times on the final code. The 13 are existing opt-in live tests, deliberately not run under the no-real-turn instruction.
- `cargo build --release`: passed, zero warnings.
- `cargo clippy --release`: passed; four existing `too_many_arguments` findings only, in `approval.rs::ask_claude`, `ledger_write.rs::create_runtime`, `native.rs::command_for_turn`, and `native.rs::run_turn`. No finding names new P5 code.
- `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py`: passed on the final release executable, including synthetic cancellation, failures, tools and the permission bridge.
- Eight scenes captured and visually inspected at 900×900, scale 1: `p5-code`, `p5-file`, `p5-recovery`, `org-palette`, each dark and light. PNGs are in `/Users/locust/work/scratch/p5-snaps/`. Palette backdrop, line highlights, pie and recovery refusal were checked. These are synthetic headless scenes, not live interaction tests.
- Logs are in `work/p5-validation/`.

Not verified: real clipboard writes/failures, live file-click interactions, platform file races, real runtime compaction, checkpoint resume, process reattachment, any real turn or a window exit test. The read-only file check uses canonical confinement but is not a descriptor-relative defence against concurrent directory replacement. The reader currently shows source lines, not a Markdown prose/versioned document surface. These are remaining implementation/verification gaps, not completed parity.

## Review and fixes

Parity pass against Electron (`ColouredCode.tsx`, `codeColors.ts`, `CopyButton.tsx`, `copyText.ts`, `FileViewer.tsx`, `ContextRing.tsx`, `compactNow.ts`, `ResumeCard.tsx`, `recoverLiveRuns.ts`, `CommandPalette.tsx` and their tests). Fixes landed in this copy only. No PARITY row was promoted to done.

### Clipboard feedback words

- Copy buttons now settle to Electron’s **Copied** / **Not copied** labels (`CopyButton.tsx` timings: 1_600 ms / 4_000 ms). Press requests `Tx::copy`, then `Tx::read_clipboard` to confirm the text landed before setting feedback; poll clears it when the hold expires (`reply_actions.rs`, `app_state.rs`, `main.rs` poll).
- Covered by `reply_actions::tests::copy_feedback_words_match_electron`. Real OS clipboard success/failure and multi-button races were not exercised live.

### Context sentence counts

- Token wording now matches Electron `usageView.compactCount` (1.5k, 1.0M roll-up, 10M, 1.0B / 1.1B), not the earlier round/truncate path that said `2k` for 1500 and never used billions (`context_usage::compact_count`).
- Covered by `compact_count_matches_electron_words` plus the existing 164k/200k sentence test.

### Recovery: no irreversible re-run

- Compared `resume.ts` / `ResumeCard.tsx`: decision cases already matched (newest checkpoint, absent/unsafe/integrity refusal, doubt names, shutdown vs manual Stop, completed/failed silence).
- `Offer::Resume` still mounts **no** Resume / Resume anyway control — native has no reconciled checkpoint execution. Doubt cards now also show Electron’s caution line (*Check those first — the next run is told to verify them before building on them.*) as words only.
- `resume_offer_never_exposes_a_resume_control` locks that no offer returns an actionable resume label. `recoverLiveRuns.ts` remains unported (host process reattachment / comparison columns).

### File reader confinement

- Extended `files_are_read_only_and_confined`: absolute paths outside the conversation folder are refused with Electron’s outside-folder sentence; `file://` targets still parse; embedded NUL paths are rejected. Reader still never launches files; footer words match FileViewer’s no-Reveal sentence.

### Left unchanged (still partial / deferred)

- Compact now / ring menu / category breakdown (`compactNow.ts`, ContextRingMenu): empty runtime-command list; sending `/compact` as a prompt is not a substitute.
- Change/diff copy and colouring; Markdown prose/version strips/Reveal/Save a copy; full Shiki grammar matrix; comparison-column live reload.
- Confirmed live clipboard against the real OS pasteboard; live window/exit; real compaction and checkpoint resume.

### Verification this review

```sh
PATH=$HOME/.cargo/bin:$PATH \
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p5 cargo test --release
# ×3 after last source edit → each: 324 passed; 0 failed; 13 ignored

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p5 cargo build --release
# zero warnings

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p5 cargo clippy --release
# four existing too_many_arguments only (approval/ledger_write/native); none in P5 review modules

scripts/test-m1-cli.py /Users/locust/work/scratch/native-target-p5/release/locust-native
scripts/test-m3a-cli.py /Users/locust/work/scratch/native-target-p5/release/locust-native
# both exit 0

# Snaps → /Users/locust/work/scratch/p5-snaps/
# p5-code, p5-file, p5-recovery, org-palette × dark/light at 900×900 scale 1
# Inspected: Copy idle label, file line highlight + no-launch footer, recovery caution
# without Resume control, palette dimmed backdrop, context 164k/200k amber sentence.
```

Logs: `work/p5-validation/` (including `test-final-*.log`, `build-final.log`, `clippy.log`, `m1-cli-final.log`, `m3a-cli-final.log`).

Not verified in this review: live window with Electron quit, live Copy against the OS pasteboard, live file-click open, real `/compact`, checkpoint resume execution, process reattachment.
