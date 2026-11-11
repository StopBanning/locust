# Locust Desktop → Native Rust Parity Checklist

## Overview & Baseline

Source audit: 2026-11-09; TypeScript CHANGELOG through 0.871.0. Every draft row was treated as unverified. Composite claims have been narrowed to supported behavior; unsupported portions are recorded under Removed (not in the app).

Path convention: renderer/, main/ and shared/ resolve under `/Users/locust/work/locust/apps/desktop/src`; packages/ and `CHANGELOG.md` resolve under `/Users/locust/work/locust`; native/ resolves under `/Users/locust/work/locust-wt/native/apps/native`. Every cited source file exists; no globs or hypothetical filenames are used.

Native reads local mission ledgers and delegates mutations/model queries to Electron's authenticated local MCP endpoint. The baseline audit predates the standalone runtimes; M4A adds generic ACP to the native Codex/Claude paths. See M4A-NOTES.md for current execution evidence. Done below includes this deliberate dependency, and means source-implemented, not live-tested.

## 1. Feature Parity by Area (15 Areas)

**done** = stated feature implemented; **partial** = subset implemented, with the gap in the cell; **missing** = no equivalent native surface or driver. Engine = requires runtime, persistence, filesystem mutation or OS integration; no = rendering/interaction over available data. Related release additions share a feature row.

Native status evidence: `native/src/main.rs` (State, message_column, view, send/stop/carry and on_key), `native/src/ledger.rs` (Item, decode, transcript assembly and Session), `native/src/engine.rs` (Engine methods) and `native/src/quick.rs` (native-only Quick Ask). Missing UI surfaces have neither corresponding view/state nor API methods in these modules. Starting an Electron-backed agent does not implement its approval, settings or development-tool UI.

### Area 1: Conversations & Thread Rendering

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Recent conversations with state dots and relative age | `renderer/src/components/Sidebar.tsx`, `main/mission-history.ts` | **done** | yes |
| Local JSONL transcript reader and live tailing (schema 1–27) | `packages/mission-store/src/index.ts`, `main/mission-history.ts` | **done** | yes |
| Continuation chain assembly | `main/conversation-chain.ts`, `renderer/src/components/Thread.tsx` | **done** | yes |
| Streaming Markdown and user message bubbles | `renderer/src/components/ThreadItems.tsx`, `renderer/src/components/Thread.tsx` | **done** | no |
| Working, waiting, ready and failure status | `renderer/src/missionView.ts`, `renderer/src/components/Thread.tsx` | **done** | no |
| Expandable tool arguments/output | `renderer/src/components/ActivityCard.tsx`, `renderer/src/components/ThreadItems.tsx` | **partial** — per-item argument/output drawers, elapsed times and bounded output expansion implemented; full Electron tool classification/group-label grammar remains | no |
| Interactive step/plan progress | `renderer/src/components/ActivityCard.tsx`, `renderer/src/components/Thread.tsx` | **partial** — expandable structured plan with live step states/counts implemented; Electron animated plan header and live-line plan peek remain | no |
| Diagnostic/rate-limit notes and cancellation receipts | `renderer/src/components/CancellationCard.tsx`, `renderer/src/ledgerFailure.ts` | **partial** — plain notes only | no |
| Watch two conversations beside one another | `renderer/src/App.tsx`, `renderer/src/components/BesideConversation.tsx` | **done** — read-only beside panel with Electron chrome words; Open it here / Close; synthetic tests and scenes | no |
| Pin, rename, settle/snooze, delete and project grouping | `renderer/src/components/Sidebar.tsx`, `renderer/src/App.tsx`, `renderer/src/conversationList.ts`, `main/mission-history.ts` | **partial** — pin/rename/settle/snooze/project membership persist via native `teammates.json`/`groups.json` (Electron read-only merge); Delete-for-good and Project settings dialog remain | yes |
| Find within conversation | `renderer/src/components/ThreadFind.tsx`, `renderer/src/threadFind.ts` | **done** — find bar with Electron words; hit count/step; no CSS Highlight glyph paint | no |
| Sidebar search by title, ID, teammate and user messages across turns | `renderer/src/components/Sidebar.tsx`, `renderer/src/status.ts` | **done** — `missionsMatching` port; Search conversations field and narrowing line | no |
| Copy reply and edit earlier message, optionally restoring files | `renderer/src/components/Thread.tsx`, `renderer/src/components/CopyButton.tsx`, `main/codex-mission.ts` | **partial** — reply Copy submits Markdown to the native clipboard; edit/restore and confirmed copy feedback remain | yes |
| Subagent/helper activity and transcript details | `renderer/src/components/ActivityCard.tsx`, `renderer/src/missionView.ts` | **partial** — nested parent-ID calls, reports, type/counts and background/end states implemented; Electron helper row formatting and full group summaries remain | no |
| Peer handoff cards and exchange budget/stop controls | `renderer/src/components/PeerThread.tsx`, `renderer/src/components/ExchangeStrip.tsx`, `main/peer-exchange.ts` | **missing** | yes |
| Decision options, handed-over files and suggested task cards | `renderer/src/components/DecisionCard.tsx`, `renderer/src/components/NextSteps.tsx`, `renderer/src/components/Thread.tsx`, `shared/protocolTags.ts` | **missing** | yes |
| Reasoning duration and folded traces | `renderer/src/components/ActivityCard.tsx`, `renderer/src/missionView.ts` | **done** — normalized public messages only, scrubbed/bounded, timed and folded; raw/legacy reasoning excluded; synthetic tests and headless scenes | no |
| Context breakdown/compact now and per-reply usage/cost | `renderer/src/components/ContextRing.tsx`, `renderer/src/components/Inspector.tsx`, `renderer/src/compactNow.ts`, `renderer/src/cost.ts` | **partial** — runtime-measured context pie and exact reading; compaction diagnostic line and invalidation; category menu, Compact now execution and cost remain | yes |
| Save Markdown record and raw JSON | `renderer/src/components/SaveRecordDialog.tsx`, `main/mission-export.ts` | **missing** | yes |
| Import Claude Code, Codex, OpenCode and Cursor conversations | `renderer/src/components/ImportDialog.tsx`, `main/session-import.ts`, `main/agent-session-import.ts` | **missing** | yes |
| Resume interrupted run from checkpoint | `renderer/src/components/ResumeCard.tsx`, `renderer/src/resume.ts` | **partial** — newest-checkpoint recovery decision and card, including named uncertain actions and refusal; reconciled resume execution remains unavailable | yes |
| Prompt rail, jump to bottom and timeline markers | `renderer/src/components/PromptRail.tsx`, `renderer/src/components/JumpToBottom.tsx`, `renderer/src/components/TimeMarker.tsx` | **missing** | no |
| Selection ask and copied read-only side chat | `renderer/src/components/SelectionAsk.tsx`, `renderer/src/components/SideChat.tsx` | **missing** | yes |
| Published-link cards and pinned page/tracker status controls | `renderer/src/components/PublishedCards.tsx`, `renderer/src/components/StatusChip.tsx`, `renderer/src/pinnedPages.ts` | **missing** | yes |
| Reply route/provenance and recovery inspector | `renderer/src/components/Inspector.tsx` | **missing** | no |

### Area 2: Composer & Input

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Send text and stop an owned turn | `renderer/src/components/Composer.tsx`, `main/locust-mcp-tools.ts` | **done** | yes |
| Restore failed-send text | `renderer/src/components/Composer.tsx` | **done** | no |
| Per-turn model and effort selection | `renderer/src/components/RoutePicker.tsx`, `renderer/src/components/EffortSlider.tsx`, `main/model-catalog.ts` | **done** | yes |
| Multiline auto-expanding editor, Shift+Enter | `renderer/src/components/Composer.tsx` | **done** | no |
| Separate conversation drafts while switching within window | `renderer/src/components/Composer.tsx` | **done** | no |
| @ file attachment and teammate tagging | `renderer/src/components/Composer.tsx`, `renderer/src/fileMentions.ts`, `main/workspace-files.ts` | **done** | yes |
| Runtime/custom slash commands and setup/usage commands | `renderer/src/components/ChatCommands.tsx`, `renderer/src/slashCommands.ts`, `main/runtime-commands.ts` | **done** | yes |
| Paste/drop files, images and PDF attachment previews | `renderer/src/components/Composer.tsx`, `renderer/src/components/AttachedImage.tsx`, `renderer/src/components/PdfTile.tsx`, `shared/attachments.ts` | **done** | yes |
| Durable busy-turn queue, edit/discard and stop/send now | `renderer/src/components/Composer.tsx`, `renderer/src/useConversationQueue.ts`, `main/queued-message-store.ts` | **done** | yes |
| Up-arrow sent-message history | `renderer/src/components/Composer.tsx` | **done** | no |
| Reference folders retained with conversation | `renderer/src/components/ReferenceChips.tsx`, `main/conversation-references.ts` | **missing** | yes |
| Permission mode, folder/own branch, Direct/Compare/Blind controls | `renderer/src/components/Composer.tsx`, `renderer/src/components/RoutePicker.tsx`, `shared/may-do.ts` | **missing** | yes |
| Review notes and browser page points attached to next message | `renderer/src/components/SentNotes.tsx`, `renderer/src/components/PagePointChips.tsx` | **missing** | yes |
| Connector-off and unavailable-route pre-send warnings | `renderer/src/components/Composer.tsx`, `renderer/src/connectorsOff.ts` | **missing** | yes |

### Area 3: Teammates, Rooms, Memory & Skills

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Teammate list with initial, role, runtime/model and busy dot | `renderer/src/components/Sidebar.tsx`, `main/teammate-store.ts` | **done** | yes |
| Select teammate and start fresh conversation by click/keyboard | `renderer/src/App.tsx`, `renderer/src/components/Sidebar.tsx` | **done** | yes |
| Create/edit/delete teammate, instructions, runtime/model/effort and budget | `renderer/src/components/NewTeammateDialog.tsx`, `renderer/src/components/InstructionsCard.tsx`, `main/teammate-store.ts` | **partial** — native-owned roster seeds once; create/edit/remove, instructions, route/mode/folder editor; Electron pickers, full validation/test coverage and budget controls remain (P6-NOTES.md) | yes |
| Starter teams and reusable routine chains | `renderer/src/components/TeamTemplates.tsx`, `shared/team-templates.ts` | **missing** | yes |
| Rooms, targeted mentions and project standing instructions | `renderer/src/components/RoomScreen.tsx`, `renderer/src/components/GroupSettingsDialog.tsx`, `main/room-store.ts`, `main/group-store.ts` | **missing** | yes |
| Automatic peer exchanges and optional handoff omissions | `main/peer-exchange.ts`, `main/handoff.ts`, `renderer/src/components/HandoffDivider.tsx` | **missing** | yes |
| Memory store, named replacements, recently forgotten and tidy suggestions | `renderer/src/components/MemoryScreen.tsx`, `renderer/src/components/MemoryCard.tsx`, `main/memory-store.ts`, `shared/memory.ts` | **partial** — native-owned Electron-shaped memory, named rewrites/proposals, edit/toggle/remove/restore and gated tidy application; full validation, recall, tidy UI and turn-card receipts remain (P6-NOTES.md) | yes |
| Workspace/runtime skills and GitHub popular/recent skill packs | `renderer/src/components/SkillsFromGitHub.tsx`, `renderer/src/components/CliArtifacts.tsx`, `main/skill-discovery.ts`, `main/skill-library.ts` | **missing** | yes |
| Teammate secret environment keys and scoped run injection | `renderer/src/components/KeyCard.tsx`, `main/teammate-keys.ts` | **missing** | yes |
| Custom faces, colours, bot/mascot and status legend | `renderer/src/components/TeammateBot.tsx`, `renderer/src/components/FaceLegend.tsx`, `renderer/src/components/PixelFace.tsx`, `renderer/src/components/AgentMark.tsx` | **missing** | no |
| About you and suggested personal instructions | `renderer/src/App.tsx`, `shared/teammate-instructions.ts` | **partial** — native About-you editor and suggestion approval/dismissal, included in native runtime prompts; full secret refusal vocabulary, change notices and TS test coverage remain (P6-NOTES.md) | yes |
| Team image import/export with routines | `renderer/src/components/TeamCard.tsx`, `main/team-card-routines.ts`, `shared/team-card.ts` | **missing** | yes |
| New teammate interview and setup inspection | `renderer/src/components/TeammateSetup.tsx`, `main/teammate-setup.ts`, `shared/interview.ts` | **missing** | yes |
| Tune teammate, combine alternatives and keep measured lessons/skills | `renderer/src/components/TuneTeammate.tsx`, `renderer/src/components/KeptInstructions.tsx`, `main/tune-lessons.ts`, `main/tune-combine.ts`, `main/tune-drafts.ts` | **missing** | yes |

### Area 4: Runtimes, Routing & Fallback

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Manual carry-on after plan/provider failure with runtime/model picker | `renderer/src/components/Thread.tsx`, `main/carry-on.ts` | **done** | yes |
| Query models and effort levels | `main/model-catalog.ts`, `main/locust-mcp-tools.ts` | **done** | yes |
| Standalone Codex app-server execution | `packages/runtime-adapters/src/codex-app-server-run.ts`, `main/codex-mission.ts` | **missing** | yes |
| Standalone Claude Code execution | `packages/runtime-adapters/src/process-runner.ts`, `packages/runtime-adapters/src/claude-events.ts` | **missing** | yes |
| Standalone Antigravity/OpenCode/Cursor/Copilot/Muse/Kilo execution | `packages/runtime-adapters/src/index.ts`, `packages/runtime-adapters/src/opencode-serve-run.ts`, `packages/runtime-adapters/src/kilo-acp-run.ts`, `main/antigravity-mission.ts` | **partial** — M4A/M4C: OpenCode 1/2, Kilo ACP, Cursor, Copilot, Antigravity (`agy`) and Muse Code run natively with continuation and Stop; no native discovery/sign-in/install UI, and live one-line CLI turns remain unrun | yes |
| Custom Agent Client Protocol agents by command | `renderer/src/components/AcpAgentsSettings.tsx`, `main/acp-agents.ts`, `packages/runtime-adapters/src/acp-run.ts` | **partial** — M4A: saved agents run over native stdio ACP, with continuation, model config, approvals and Stop; no native agent settings/Test/save/remove or capability display | yes |
| Own OpenAI-compatible API/local models, pricing and chat-only capability | `renderer/src/components/OwnModels.tsx`, `main/own-models.ts`, `main/listed-prices.ts` | **missing** | yes |
| Automatic carry-on policy | `main/carry-on.ts`, `shared/carry-on.ts` | **missing** | yes |
| Named Codex/Claude accounts and explicit account selection | `renderer/src/components/AgentAccountsSettings.tsx`, `main/agent-accounts.ts` | **missing** | yes |
| Agent discovery (including Gemini), install, sign-in, updates and capability reporting | `renderer/src/components/SignInButton.tsx`, `main/runtime-discovery.ts`, `main/runtime-installer.ts`, `main/runtime-setup.ts`, `main/runtime-sign-in.ts`, `main/runtime-updates.ts`, `shared/runtimes.ts` | **missing** | yes |
| Cross-runtime handoff briefing and omission choices | `main/handoff.ts`, `renderer/src/components/HandoffDivider.tsx` | **partial** — delegates briefing to Electron; no omission UI | yes |

### Area 5: Approvals & Permissions

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Action approval/question cards, remembered answers and denial reasons | `renderer/src/components/ApprovalCard.tsx`, `main/approval-channel.ts`, `main/approval-rule-store.ts` | **partial** (P8: Approve/Deny/Always/Don't-ask-again/Never-allow card, reach/In short rows, saved-rule auto-answer, command-guard hard deny; no question form yet) | yes |
| Command safety/reach guard and In short explanation | `main/command-guard.ts`, `shared/command-reach.ts`, `shared/command-plain.ts` | **partial** — Rust `command_reach`/`command_plain`/`command_guard` with Electron words; OpenCode still ships the JS guard asset; full PreToolUse writers for every runtime remain (P8-NOTES.md) | yes |
| Workspace boundary, outside attachments and agent-settings-folder confirmation | `main/permission-host.ts`, `main/attach-outside.ts`, `main/workspace-settings-change.ts` | **partial** — outside-folder card words, `attach_outside` destination/exclude, `settings_change_kept`; full permission-host connector/browser cards remain (P8-NOTES.md) | yes |
| Connector/browser action permissions | `main/permission-host.ts`, `main/browser-host.ts` | **missing** | yes |
| Saved approval rules list/remove/remove all | `renderer/src/components/SavedApprovalRules.tsx`, `main/approval-rule-store.ts` | **done** — native `approval-rules.json` seed-once, list/remove/remove all, secret/compound/reach refusals; Settings panel words exact | yes |
| Ask/Plan/Auto/Approve each action and runtime-specific guarantees | `main/mission-mode.ts`, `main/permission-host.ts`, `renderer/src/components/WhatLocustCanStop.tsx` | **partial** (M3b: `codexAppServerPolicy` + Approve-each interactive for Codex/Claude control_request; Ask auto-declines; Claude permission bridge / full mode matrix later) | yes |
| Scoped secret detection/redaction in memories, records and sharing | `shared/secrets.ts`, `main/mission-export.ts`, `main/memory-store.ts` | **partial** — one `secrets` module (`secret_in`/`scrub_secrets`/`redact_secrets`); people/ledger/events call it; Save-record export UI remains (P8-NOTES.md) | yes |
| Approval alerts on Board/sidebar/taskbar/phone/chat | `renderer/src/components/BoardScreen.tsx`, `renderer/src/components/Sidebar.tsx`, `main/raised-approvals.ts`, `main/taskbar-attention.ts` | **missing** | yes |
| Organization machine policy and spend limits | `main/organization-policy.ts`, `main/organization-spend.ts`, `shared/organization-policy.ts` | **missing** | yes |

### Area 6: Changes, Git, Worktrees, Issues & Scout

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Folder-wide Changes panel, file kinds and line counts | `renderer/src/components/ChangesPanel.tsx`, `renderer/src/components/FileTypeIcon.tsx`, `renderer/src/components/LineCounts.tsx`, `main/recent-edits.ts` | **partial** — native branch/base read-only panel, file list/kinds/counts, commit selection; folder tree, full icon/menu/probe parity remain (P7-NOTES.md) | yes |
| Coloured unified diffs, expandable hunks and copy change/file | `renderer/src/components/DiffView.tsx`, `renderer/src/diff.ts`, `renderer/src/diffColours.ts` | **partial** — native syntax-coloured unified rows, hunk/context expansion and both copy controls; word marks, document passages and complete TS coverage remain (P7-NOTES.md) | no |
| Undo turn, including non-Git folder changes | `renderer/src/components/TurnUndo.tsx`, `main/checkpoints.ts`, `main/checkpoints-plain.ts` | **partial** — native before/after byte checkpoints, confirmed guarded restore and receipts; Electron storage/retention, turn-foot/card integration and runtime notice remain (P7-NOTES.md) | yes |
| Commit composer and generated commit message | `renderer/src/components/CommitChanges.tsx`, `main/folder-commit.ts` | **missing** | yes |
| Own-branch turn commits and worktree lifecycle | `renderer/src/components/WorktreeRow.tsx`, `main/worktrees.ts`, `main/turn-checkpoint.ts` | **missing** | yes |
| Review/land branch as one commit and return conflicts to teammate | `renderer/src/components/ReviewChanges.tsx`, `main/worktrees.ts` | **missing** | yes |
| GitHub PR header/link state, checks and watched news | `renderer/src/components/PullRequestChip.tsx`, `renderer/src/components/PullRequestNewsBanner.tsx`, `main/pull-request-watch.ts`, `main/pull-link.ts` | **missing** | yes |
| Browse GitHub issues and start conversation/branch | `renderer/src/components/IssueList.tsx`, `main/github-issues.ts` | **missing** | yes |
| Post-edit check and fix-until-check-passes | `renderer/src/components/EditCheckCard.tsx`, `renderer/src/components/CheckFixLine.tsx`, `main/edit-check.ts`, `main/check-fix.ts` | **missing** | yes |
| Scout watch/checks, findings, notes, morning brief and weekly tune-up | `renderer/src/components/ScoutCards.tsx`, `renderer/src/components/ScoutSetting.tsx`, `main/scout.ts`, `main/scout-folder.ts` | **missing** | yes |
| Scout /locust comments, away GitHub workflow and phone cards | `main/scout-comments.ts`, `main/scout-away.ts`, `main/phone-scout-cards.ts` | **missing** | yes |
| Scout prepares fixes in copies for review | `main/scout-fix.ts`, `renderer/src/components/ScoutCards.tsx` | **missing** | yes |
| Diff annotations and walk me through changes | `renderer/src/components/DiffNotes.tsx`, `renderer/src/components/SentNotes.tsx`, `renderer/src/components/ChangesPanel.tsx`, `shared/walkthrough.ts` | **missing** | yes |

### Area 7: Terminal & Own Commands

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Interactive multi-tab resizable PTY panel beside conversation | `renderer/src/components/TerminalPanel.tsx`, `renderer/src/components/TerminalToggle.tsx`, `main/terminal-host.ts`, `main/pseudo-terminal.ts` | **partial** — real PTY, tabs, drag resize, ANSI cells/cursor, scrollback/selection/copy/paste; terminal replies, full key/mouse/accessibility and TS lifecycle matrix remain (P9-NOTES.md) | yes |
| Run own ! command and record output in thread | `renderer/src/components/OwnCommand.tsx`, `main/own-command-host.ts`, `main/own-command-ipc.ts` | **partial** — typed ! runs in selected folder without agent approvals; stop, bounded output, local thread entry and once-only next-message carry; exact command UI/test coverage remains (P9-NOTES.md) | yes |
| Open runtime terminal and catch up its conversation turns | `renderer/src/components/OpenInTerminal.tsx`, `renderer/src/components/TerminalDivider.tsx`, `main/open-in-terminal.ts`, `main/terminal-catch-up.ts` | **partial** — macOS script/resume and explicit Claude/Codex catch-up; automatic focus/pre-send catch-up, ownership/account parity and full TS tests remain (P9-NOTES.md) | yes |

### Area 8: Built-in Browser & Page Previews

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Browser tabs, URL/navigation, reload and viewport | `renderer/src/components/BrowserPanel.tsx`, `renderer/src/components/BrowserToggle.tsx`, `main/browser-host.ts` | **missing** | yes |
| Local page preview and comparison page previews | `main/page-preview.ts`, `renderer/src/components/FileViewer.tsx`, `renderer/src/components/CompareView.tsx` | **missing** | yes |
| Point at page/browser element and attach screenshot/context | `main/browser-point.ts`, `main/page-pick.ts`, `renderer/src/components/PagePointChips.tsx` | **missing** | yes |
| Teammate browser tools with permission gates | `main/browser-host.ts`, `main/browser-ipc.ts` | **missing** | yes |
| Browser console/network logs readable by teammate tools | `main/browser-host.ts` | **missing** | yes |
| Separate teammate browser sign-in profiles | `main/browser-host.ts`, `renderer/src/components/BrowserPanel.tsx` | **missing** | yes |

### Area 9: Routines, Compare & Benchmarks

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Routine editor, schedules/days/one-off, pause and run history | `renderer/src/components/AutomationsScreen.tsx`, `renderer/src/components/RoutineDialog.tsx`, `main/routine-runner.ts`, `main/routine-store.ts` | **missing** | yes |
| Arriving-file routine trigger | `main/routine-file-watch.ts` | **missing** | yes |
| Private routine link hook | `main/routine-hooks.ts`, `shared/routine-hook.ts` | **missing** | yes |
| Routine input forms, Markdown import/export and starter templates | `renderer/src/components/RoutineInputs.tsx`, `renderer/src/components/RoutineFileDialog.tsx`, `renderer/src/components/RoutineTemplates.tsx`, `main/routine-file.ts`, `main/routine-templates.ts` | **missing** | yes |
| Routine copy-mode changes awaiting keep/discard | `renderer/src/components/RoutineChanges.tsx`, `main/routine-copy.ts` | **missing** | yes |
| Routine retry-until-check, handoff steps and wait-for-OK gates | `renderer/src/components/RoutineRecovery.tsx`, `main/routine-recovery-ipc.ts`, `main/routine-runner.ts` | **missing** | yes |
| Compare/Blind side-by-side answers and isolated changes | `renderer/src/components/CompareView.tsx`, `main/compare-store.ts`, `main/compare-copies.ts` | **missing** | yes |
| Weighted/drafted rubric points and one/three blind judges | `renderer/src/components/CompareView.tsx`, `main/compare-judge.ts`, `shared/compare-score.ts` | **missing** | yes |
| Per-column rerun, build/merge answers, keep copies and preview pages | `renderer/src/components/CompareView.tsx`, `main/compare-reruns.ts`, `main/compare-copies.ts` | **missing** | yes |
| Reusable benchmarks, record tables and across-benchmark analysis | `renderer/src/components/CompareBenchmark.tsx`, `renderer/src/components/BenchmarksScreen.tsx`, `renderer/src/benchmark.ts`, `renderer/src/compareRecord.ts` | **missing** | yes |

### Area 10: Calls & Voice Typing

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Voice dictation in composer | `renderer/src/components/VoiceButton.tsx`, `main/voice-host.ts`, `renderer/src/voiceCapture.ts` | **missing** | yes |
| Realtime ChatGPT voice through Codex and teammate calls | `renderer/src/components/VoiceCall.tsx`, `main/voice-call.ts`, `main/voice-openai.ts` | **missing** | yes |
| Saved call transcript, teammate handoffs and summary | `renderer/src/components/CallWords.tsx`, `main/call-log.ts`, `renderer/src/callTodos.ts` | **missing** | yes |
| Windows global Talk shortcut, voice settings and greeting preview | `renderer/src/components/TalkShortcutSetting.tsx`, `renderer/src/components/TalkVoice.tsx`, `renderer/src/components/VoicePlay.tsx`, `renderer/src/components/VoiceSettings.tsx`, `main/talk-shortcut.ts`, `main/voice-preview.ts` | **missing** | yes |
| Call folder and connector context | `main/voice-call-folder.ts`, `main/voice-call.ts` | **missing** | yes |

### Area 11: Settings, Usage, Privacy & Backup

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Categorized Settings and project settings | `renderer/src/components/Screens.tsx`, `renderer/src/settingsPages.ts`, `main/workspace.ts` | **missing** | yes |
| Usage/cost across agents/models/accounts, plan meters and budgets | `renderer/src/components/UsageDialog.tsx`, `renderer/src/components/UsageLine.tsx`, `renderer/src/components/UsageMeters.tsx`, `shared/usage.ts`, `shared/spend.ts` | **missing** | yes |
| OS keychain/safeStorage and unlock note | `main/keychain.ts`, `renderer/src/components/KeychainNote.tsx` | **missing** | yes |
| Retention and manifest-verified local profile backup/restore | `main/retention.ts`, `main/profile-backup.ts` | **missing** | yes |
| Connector inventory, health and add to supported agents | `renderer/src/components/ConnectorHealth.tsx`, `renderer/src/components/AddConnector.tsx`, `main/connector-reader.ts`, `main/connector-add.ts` | **missing** | yes |
| Feedback and private report export/send controls | `renderer/src/components/FeedbackDialog.tsx`, `main/report-problem.ts` | **missing** | yes |
| Keyboard shortcut reference | `renderer/src/components/KeyboardSheet.tsx`, `renderer/src/keyboardShortcuts.ts` | **missing** | no |
| GitHub account and CLI readiness | `renderer/src/components/GitHubAccount.tsx`, `main/github-account.ts` | **missing** | yes |

### Area 12: Cloud, Remote, Phone & Chat Apps

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Phone Wi-Fi pairing and encrypted anywhere relay | `renderer/src/components/PhoneSettings.tsx`, `main/phone-host.ts`, `main/phone-anywhere.ts`, `main/relay-crypto.ts` | **missing** | yes |
| Phone approvals, quiet hours/alerts and sealed offline message queue | `main/phone-alerts.ts`, `main/phone-page.ts`, `main/web-push.ts` | **missing** | yes |
| Full window in browser with code/GitHub sign-in | `main/desk-host.ts`, `main/desk-page.ts`, `main/github-account.ts` | **missing** | yes |
| Cloud computer, agent sign-in, hours and remote turns | `renderer/src/components/CloudComputers.tsx`, `renderer/src/components/CloudSetupDialog.tsx`, `renderer/src/components/CloudTurn.tsx`, `main/cloud-computer.ts`, `main/cloud-turns.ts` | **missing** | yes |
| Cloud task changes/PRs, Claude cloud and background runs | `renderer/src/components/CloudTasks.tsx`, `renderer/src/components/BackgroundRuns.tsx`, `main/cloud-tasks.ts`, `main/claude-cloud.ts`, `main/claude-background-runs.ts` | **missing** | yes |
| Telegram/Discord/Slack pairing and approval notifications | `renderer/src/components/ChatAppsSettings.tsx`, `renderer/src/components/TelegramSettings.tsx`, `main/telegram.ts`, `main/discord.ts`, `main/slack.ts` | **missing** | yes |
| Remote MCP app pairing, in-window grants and revocation | `renderer/src/components/RemoteMcpSettings.tsx`, `renderer/src/components/RemoteConnectCard.tsx`, `main/remote-mcp.ts` | **missing** | yes |
| GitHub account sealed profile sync, recovery and transfer | `renderer/src/components/YourAccountSettings.tsx`, `main/your-account.ts`, `main/profile-sync.ts`, `main/profile-sync-crypto.ts` | **missing** | yes |
| Claude Code remote control and background access | `renderer/src/components/RemoteControlSetting.tsx`, `main/claude-remote-control.ts`, `main/claude-background.ts` | **missing** | yes |

### Area 13: Onboarding, Home, Updates & CLI

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Local MCP teammates/conversations/models/stop/carry-on access | `renderer/src/components/LocustMcpSettings.tsx`, `main/locust-mcp-host.ts`, `main/locust-mcp-tools.ts` | **partial** — client only; no native server/settings/routine tools | yes |
| First launch, discovery and starter team setup | `renderer/src/components/FirstLaunch.tsx`, `renderer/src/components/BootScreen.tsx`, `main/start-readiness.ts` | **missing** | yes |
| Folder/project switcher and remembered conversation folders | `renderer/src/components/Sidebar.tsx`, `main/folders.ts`, `main/named-folder.ts` | **missing** | yes |
| Home cover, team, starters and compare entry points | `renderer/src/components/HomeCover.tsx`, `renderer/src/components/HomeTeam.tsx`, `renderer/src/components/StarterList.tsx`, `renderer/src/components/IdleTeammate.tsx` | **missing** | yes |
| Board of waiting/working/done with actionable approvals | `renderer/src/components/BoardScreen.tsx`, `renderer/src/conversationBoard.ts` | **missing** | yes |
| Since-you-were-away recap | `renderer/src/components/AwayList.tsx`, `main/away.ts` | **missing** | yes |
| Updates, release notes and big-update splash | `renderer/src/components/WhatsNew.tsx`, `renderer/src/components/Screens.tsx`, `main/updates.ts`, `main/mac-self-update.ts`, `main/linux-install.ts` | **missing** | yes |
| Global command palette | `renderer/src/components/CommandPalette.tsx` | **done** — Cmd/Ctrl+K; Electron placeholder/groups/labels; filter + group order | no |
| External CLI client for local MCP tools | `packages/cli/src/index.ts` | **partial** — native --ask covers ask/reply only | yes |
| Expose team as ACP agent to editors, with opt-in own-turn approval answers | `packages/cli/src/acp.ts`, `main/locust-cli-sessions.ts` | **missing** | yes |
| One-click Claude desktop MCP connection and restart/remove | `main/claude-app-connection.ts`, `renderer/src/components/LocustMcpSettings.tsx` | **missing** | yes |
| Deep links prefill message/project | `main/locust-links.ts`, `shared/locust-link.ts` | **missing** | yes |
| Startup/login item, keep running/tray and keep awake | `main/login-item.ts`, `main/quit-guard.ts`, `main/keep-running.ts`, `main/keep-awake.ts`, `main/index.ts` | **missing** | yes |

### Area 14: Pets, Bots & Look / Window Chrome

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Dark/light palette following computer | `renderer/src/components/ThemeSetting.tsx`, `main/appearance.ts` | **done** | no |
| Saved appearance choice and window look settings | `renderer/src/components/ThemeSetting.tsx`, `renderer/src/components/LookSetting.tsx`, `main/appearance.ts` | **missing** | yes |
| macOS/Windows liquid glass and rainbow backdrop | `main/mac-glass.ts`, `main/windows-glass.ts`, `renderer/src/components/LookSetting.tsx` | **missing** | yes |
| Pet gallery, kept pets, sprites/puppets and bulk removal | `renderer/src/components/PetPicks.tsx`, `renderer/src/components/PetSprite.tsx`, `renderer/src/components/PetPuppet.tsx`, `main/pet-library.ts` | **missing** | yes |
| Animated bots, plush/terminal eyes and agent mascots | `renderer/src/components/Bot.tsx`, `renderer/src/components/TeammateBot.tsx`, `renderer/src/buddyRig.ts`, `renderer/src/botLook.ts` | **missing** | no |
| Native titlebar/dragging and collapsible compact sidebar | `renderer/src/components/TitleBar.tsx`, `renderer/src/components/Sidebar.tsx`, `main/window-chrome.ts` | **partial** — drag regions present; fixed-width sidebar | no |

### Area 15: Documents, Diagrams & File Viewer

| Feature | Where in TS | Native status | Depends on engine |
| :--- | :--- | :--- | :---: |
| Coloured code blocks, copy and file path controls | `renderer/src/components/ThreadItems.tsx`, `renderer/src/components/ColouredCode.tsx`, `renderer/src/components/CopyButton.tsx` | **partial** — Rune syntax colours, code/reply Copy and file-path controls; full Electron grammars, confirmed clipboard feedback and change-copy semantics remain | no |
| File viewer, line targeting and context menu/editor/reveal actions | `renderer/src/components/FileViewer.tsx`, `main/reveal-file.ts`, `shared/file-menu.ts` | **partial** — bounded read-only UTF-8 reader with targeted lines, close and whole-file Copy; Markdown mode, version history, context menu/editor/reveal/save remain | yes |
| Mermaid diagram/source view | `renderer/src/components/ReplyDiagram.tsx`, `main/diagrams.ts`, `main/diagram-view.ts` | **missing** | yes |
| PDF pages, attachment thumbnail and text reading | `renderer/src/components/PdfPages.tsx`, `renderer/src/components/PdfTile.tsx`, `main/pdf-pages.ts`, `main/pdf-reading.ts` | **missing** | yes |
| Word/PowerPoint reading, document changes, XLSX/CSV sheets and image zoom | `renderer/src/components/DocPreview.tsx`, `renderer/src/components/DocumentView.tsx`, `renderer/src/components/DocumentChange.tsx`, `renderer/src/components/SheetView.tsx`, `renderer/src/components/ZoomableImage.tsx`, `main/office-words.ts`, `main/xlsx.ts` | **missing** | yes |
| Runnable HTML reply pages and source/preview view | `renderer/src/components/ReplyPage.tsx`, `main/reply-page.ts`, `main/page-preview.ts` | **missing** | yes |
| Rendered math in replies | `renderer/src/components/MathTex.tsx`, `renderer/src/components/ThreadItems.tsx` | **missing** | no |

## 2. Engine Breakdown: Main-Process Services & Standalone Requirements

Real service entry points below replace the draft's invented names. These remain Electron-side; an MCP client is not a native implementation. This inventory overlaps feature areas and is not a mutually exclusive line allocation.

| Subsystem | Existing TS entry points | Needed for standalone Rust |
| :--- | :--- | :--- |
| Runtime execution | `main/codex-mission.ts`, `main/antigravity-mission.ts`, `main/app-server-process.ts`, `packages/runtime-adapters/src/index.ts`, `packages/runtime-adapters/src/process-runner.ts`, `packages/runtime-adapters/src/acp-run.ts` | Subprocess/protocol drivers and normalized streaming. |
| Ledger/history/export | `main/ledger-writes.ts`, `main/mission-history.ts`, `main/conversation-chain.ts`, `main/mission-export.ts`, `packages/mission-store/src/index.ts` | Ledger writer/recovery, indexes, continuations, import and saved records. |
| Turn lifecycle/carry-on | `main/codex-mission.ts`, `main/run-end.ts`, `main/carry-on.ts`, `main/handoff.ts`, `main/queued-message-store.ts` | Briefing, queue persistence, failures and continuation dispatch. |
| Models/accounts/setup | `main/model-catalog.ts`, `main/agent-accounts.ts`, `main/runtime-discovery.ts`, `main/runtime-setup.ts`, `main/runtime-sign-in.ts`, `main/own-models.ts` | Catalog, account isolation, install/auth and own endpoint client. |
| Team/rooms/memory/skills/relay | `main/teammate-store.ts`, `main/room-store.ts`, `main/group-store.ts`, `main/memory-store.ts`, `main/skill-discovery.ts`, `main/skill-library.ts`, `main/peer-exchange.ts`, `main/teammate-keys.ts` | Persistent stores, instructions, scoped keys and bounded peer dispatch. |
| Approvals/guard/policy | `main/approval-channel.ts`, `main/approval-rule-store.ts`, `main/permission-host.ts`, `main/command-guard.ts`, `main/organization-policy.ts`, `main/organization-spend.ts` | Pause/resume, saved rules, command checks and organization enforcement. |
| Git/checkpoints/GitHub/Scout | `main/recent-edits.ts`, `main/worktrees.ts`, `main/turn-checkpoint.ts`, `main/checkpoints.ts`, `main/folder-commit.ts`, `main/pull-request-watch.ts`, `main/github-issues.ts`, `main/edit-check.ts`, `main/scout.ts` | Observation, undo, branch landing, GitHub metadata and Scout jobs. |
| Routines/compare/tune | `main/routine-runner.ts`, `main/routine-store.ts`, `main/routine-file-watch.ts`, `main/routine-hooks.ts`, `main/compare-store.ts`, `main/compare-copies.ts`, `main/compare-judge.ts`, `main/compare-reruns.ts`, `main/tune-lessons.ts` | Scheduling, file/link triggers, reviewed steps/copies, scoring and tuning. |
| Terminals/own commands | `main/terminal-host.ts`, `main/pseudo-terminal.ts`, `main/own-command-host.ts`, `main/terminal-catch-up.ts` | PTY lifecycle, command recording and terminal session catch-up. |
| Browser/pages | `main/browser-host.ts`, `main/browser-ipc.ts`, `main/browser-point.ts`, `main/page-preview.ts`, `main/page-pick.ts`, `main/reply-page.ts` | Isolated tabs/profiles, permission bridge, capture and sandboxed previews. |
| Voice/calls | `main/voice-host.ts`, `main/voice-call.ts`, `main/voice-openai.ts`, `main/voice-call-folder.ts`, `main/call-log.ts`, `main/talk-shortcut.ts` | Capture/transcription, realtime voice, transcripts/handoffs and OS shortcut. |
| Settings/security/backup/connectors | `main/workspace.ts`, `main/keychain.ts`, `main/retention.ts`, `main/profile-backup.ts`, `main/connector-reader.ts`, `main/connector-add.ts` | Settings/key storage, manifest-verified backup and connector inspection/installation. |
| Phone/desk/chat/cloud/sync | `main/phone-host.ts`, `main/phone-anywhere.ts`, `main/desk-host.ts`, `main/remote-mcp.ts`, `main/telegram.ts`, `main/discord.ts`, `main/slack.ts`, `main/cloud-computer.ts`, `main/cloud-turns.ts`, `main/cloud-tasks.ts`, `main/claude-cloud.ts`, `main/profile-sync.ts` | Encrypted pairing/relay, browser access, app grants, chat/cloud transports and sync. |
| MCP/startup/update | `main/index.ts`, `main/locust-mcp-host.ts`, `main/locust-mcp-tools.ts`, `main/login-item.ts`, `main/quit-guard.ts`, `main/updates.ts`, `main/mac-self-update.ts`, `main/linux-install.ts` | Authenticated server, lifecycle/tray/login and platform updater. |
| Window/pets/documents | `main/window-chrome.ts`, `main/mac-glass.ts`, `main/windows-glass.ts`, `main/pet-library.ts`, `main/pdf-pages.ts`, `main/office-words.ts`, `main/xlsx.ts`, `main/diagrams.ts`, `main/reveal-file.ts` | Window materials, pet storage and safe document/file/diagram readers. |

## 3. Totals

Computed from Section 1 only; removed claims, engine inventory and native-only utilities are excluded. Partial does not count as done.

| # | Area | Done | Partial | Missing | Total |
| :--- | :--- | ---: | ---: | ---: | ---: |
| 1 | Conversations & Thread Rendering | 9 | 8 | 8 | 25 |
| 2 | Composer & Input | 10 | 0 | 4 | 14 |
| 3 | Teammates, Rooms, Memory & Skills | 2 | 3 | 9 | 14 |
| 4 | Runtimes, Routing & Fallback | 2 | 3 | 6 | 11 |
| 5 | Approvals & Permissions | 1 | 5 | 3 | 9 |
| 6 | Changes, Git, Worktrees, Issues & Scout | 0 | 3 | 10 | 13 |
| 7 | Terminal & Own Commands | 0 | 3 | 0 | 3 |
| 8 | Built-in Browser & Page Previews | 0 | 0 | 6 | 6 |
| 9 | Routines, Compare & Benchmarks | 0 | 0 | 10 | 10 |
| 10 | Calls & Voice Typing | 0 | 0 | 5 | 5 |
| 11 | Settings, Usage, Privacy & Backup | 0 | 0 | 8 | 8 |
| 12 | Cloud, Remote, Phone & Chat Apps | 0 | 0 | 9 | 9 |
| 13 | Onboarding, Home, Updates & CLI | 1 | 2 | 10 | 13 |
| 14 | Pets, Bots & Look / Window Chrome | 1 | 1 | 4 | 6 |
| 15 | Documents, Diagrams & File Viewer | 0 | 2 | 5 | 7 |
| | **TOTAL** | **26** | **30** | **97** | **153** |

Source-size context: physical lines including blanks/comments; TypeScript excludes test/spec/declaration files. Rust production lines stop before each trailing cfg(test) module. Files serve multiple areas, so the draft's exact per-area allocations and additive engine counts have been withdrawn.

| Source group | Files | Production lines | Total lines |
| :--- | ---: | ---: | ---: |
| Desktop renderer | 314 | 85,401 | 85,401 |
| Desktop shared | 128 | 24,998 | 24,998 |
| Desktop main | 253 | 72,836 | 72,836 |
| Desktop preload | 1 | 38 | 38 |
| Runtime adapters | 32 | 18,212 | 18,212 |
| Mission store | 4 | 3,735 | 3,735 |
| CLI | 4 | 493 | 493 |
| Native app | 4 | 4,330 | 6,471 |

## 4. Suggested Rounds (Daily Path First)

Estimates are **rough additional production Rust LOC**, not measured sizes or one-day guarantees. Split rounds when integration/testing requires it. Existing edit targets are `native/src/main.rs`, `native/src/ledger.rs`, `native/src/engine.rs` and `native/src/quick.rs`; future modules are deliberately unnamed. TS references are source guides, not files to modify.

| Round | Scope in order | Existing source guides | Rough Rust LOC |
| :--- | :--- | :--- | ---: |
| 1 | Rich tools/helpers, reasoning and interactive plans | `renderer/src/components/ActivityCard.tsx`, `renderer/src/components/ThreadItems.tsx` | ~1,200 |
| 2 | Multiline, window drafts, attachments, mentions/commands and durable queue | `renderer/src/components/Composer.tsx`, `main/queued-message-store.ts` | ~1,300 |
| 3 | Approval/question responses, saved rules and command explanations | `renderer/src/components/ApprovalCard.tsx`, `main/approval-channel.ts`, `main/approval-rule-store.ts` | ~1,100 |
| 4 | Conversation organization, find/palette, prompt rail and beside view | `renderer/src/components/Sidebar.tsx`, `renderer/src/components/ThreadFind.tsx`, `renderer/src/components/CommandPalette.tsx`, `renderer/src/components/BesideConversation.tsx` | ~1,300 |
| 5 | Copy/code/file actions, context usage and interrupted-run recovery | `renderer/src/components/FileViewer.tsx`, `renderer/src/components/ThreadItems.tsx`, `renderer/src/components/ContextRing.tsx`, `renderer/src/components/ResumeCard.tsx` | ~1,100 |
| 6 | Native ledger writer/history and authenticated local MCP server | `main/ledger-writes.ts`, `main/locust-mcp-host.ts`, `packages/mission-store/src/index.ts` | ~1,400 |
| 7 | Team/project/memory/skills stores and teammate editing | `main/teammate-store.ts`, `main/memory-store.ts`, `main/skill-discovery.ts`, `renderer/src/components/NewTeammateDialog.tsx` | ~1,200 |
| 8 | Standalone Codex and Claude Code drivers | `packages/runtime-adapters/src/codex-app-server-run.ts`, `packages/runtime-adapters/src/process-runner.ts` | ~1,500 |
| 9 | Supported Antigravity/OpenCode/Cursor/Copilot/Muse/Kilo/ACP and own-model paths | `packages/runtime-adapters/src/index.ts`, `main/own-models.ts`, `main/acp-agents.ts` | ~1,400 |
| 10 | Catalog, account selection and manual/automatic carry-on briefing | `main/model-catalog.ts`, `main/agent-accounts.ts`, `main/carry-on.ts`, `main/handoff.ts` | ~1,300 |
| 11 | Guard, permission enforcement, organization limits and key storage | `main/command-guard.ts`, `main/permission-host.ts`, `main/organization-policy.ts`, `main/keychain.ts` | ~1,200 |
| 12 | Rooms, peer dispatch and share/decision/file/task actions | `main/peer-exchange.ts`, `main/room-store.ts`, `renderer/src/components/DecisionCard.tsx`, `renderer/src/components/NextSteps.tsx` | ~1,200 |
| 13 | Changes, coloured diffs, notes and undo | `renderer/src/components/ChangesPanel.tsx`, `renderer/src/components/DiffView.tsx`, `renderer/src/components/DiffNotes.tsx`, `main/checkpoints.ts` | ~1,400 |
| 14 | Commit/own branch, review/land and check/fix loops | `renderer/src/components/CommitChanges.tsx`, `renderer/src/components/ReviewChanges.tsx`, `main/worktrees.ts`, `main/check-fix.ts` | ~1,400 |
| 15 | GitHub issues/PR watching and Scout watch/fix/away | `main/github-issues.ts`, `main/pull-request-watch.ts`, `main/scout.ts`, `main/scout-away.ts` | ~1,200 |
| 16 | PTY, own commands and terminal catch-up | `renderer/src/components/TerminalPanel.tsx`, `main/own-command-host.ts`, `main/terminal-catch-up.ts` | ~1,300 |
| 17 | Browser tabs/profiles, page previews and element capture | `renderer/src/components/BrowserPanel.tsx`, `main/browser-host.ts`, `main/page-pick.ts` | ~1,300 |
| 18 | Routine schedules/file/link triggers, inputs/templates, gates and reviewed copies | `main/routine-runner.ts`, `main/routine-hooks.ts`, `renderer/src/components/RoutineChanges.tsx`, `renderer/src/components/RoutineRecovery.tsx` | ~1,300 |
| 19 | Compare/blind, judges/reruns, benchmarks and measured tuning | `renderer/src/components/CompareView.tsx`, `renderer/src/components/BenchmarksScreen.tsx`, `renderer/src/components/TuneTeammate.tsx` | ~1,300 |
| 20 | Settings, usage, connectors, backup and feedback | `renderer/src/components/Screens.tsx`, `renderer/src/components/UsageDialog.tsx`, `renderer/src/components/AddConnector.tsx`, `main/profile-backup.ts` | ~1,400 |
| 21 | Dictation/realtime calls, transcripts and voice selection | `renderer/src/components/VoiceButton.tsx`, `renderer/src/components/VoiceCall.tsx`, `main/call-log.ts`, `renderer/src/components/TalkVoice.tsx` | ~1,400 |
| 22 | Phone/browser/remote MCP and paired chat apps | `main/phone-anywhere.ts`, `main/desk-host.ts`, `main/remote-mcp.ts`, `renderer/src/components/ChatAppsSettings.tsx` | ~1,400 |
| 23 | Cloud computer/tasks/background runs and sealed profile sync | `renderer/src/components/CloudComputers.tsx`, `renderer/src/components/CloudTasks.tsx`, `renderer/src/components/YourAccountSettings.tsx` | ~1,300 |
| 24 | PDF, Word/PowerPoint, sheets/images, math/Mermaid and reply pages | `renderer/src/components/DocPreview.tsx`, `renderer/src/components/PdfPages.tsx`, `renderer/src/components/ReplyDiagram.tsx`, `renderer/src/components/ReplyPage.tsx` | ~1,200 |
| 25 | First launch/Home/Board, updates, startup/tray, glass and pets | `renderer/src/components/FirstLaunch.tsx`, `renderer/src/components/HomeCover.tsx`, `renderer/src/components/BoardScreen.tsx`, `main/updates.ts`, `main/quit-guard.ts`, `renderer/src/components/PetPicks.tsx` | ~1,300 |

## Removed (not in the app)

These are removed draft rows or unsupported clauses of composite rows. Verified subsets remain in Section 1. A missing filename alone was not treated as evidence that a feature is absent.

- Global Quick Ask, hidden-session tracking and screenshot/selection capture/open-main controls: native-only; no TS Quick Ask window. Global Talk is a distinct real feature.
- UI frame-time/memory benchmark and --snap visual scene CLI: native engineering utilities, not TS user features. Quick Ask and native --bench/--snap do not earn TS parity credit.
- Task-classification/model-tier smart router: no corresponding app service. Route selection and carry-on are real.
- Automatic multi-account quota rotation: named account creation and explicit selection exist, not that policy.
- Claimed built-in Gemini mission execution and Aider/Goose/Amp/Qwen/Droid/Windsurf adapter suite: Gemini is discovered and supports sign-in, but shared runtime capability code explicitly refuses missions because it lacks an event reader. Custom ACP support is real, not proof of separate built-in integrations for the other names.
- Teammate SSH/GPG signing keys: real keys are secret environment values.
- Search over all assistant transcript text: sidebar search covers titles, IDs, teammates and the person's messages across turns, and in-thread find is real; no global assistant-body search engine found.
- Conversation tabs/archive suite and message bookmark/pin/fork suite as described: retain real pin/rename/settle/snooze/delete/project grouping, copy/edit and side-session features.
- HTML/shareable-link conversation export: actual saved record is Markdown plus raw JSON.
- Assistant-message read-aloud: voice greeting preview and realtime Talk exist, not that control.
- Draft persistence across restarts: composer drafts live in a window-local Map; queued messages persist separately.
- Saved prompt/snippet library and symbol-autocomplete suite: file mentions, custom slash commands and Up-arrow history exist.
- Arbitrary domain allowlist settings and universal outbound-prompt/tool-output secret scanner: actual checks are scoped connector/browser permissions and secret detection/redaction.
- Split diff with inline hunk stage/revert and generic branch switch/push/pull panel: actual workflows are unified diffs, commit/review/land and undo.
- GitLab PR create/review/merge and Linear integration: actual app surfaces use GitHub issues/PR metadata/watching and cloud task PR actions.
- Attach terminal selection and daemon/ports/restart manager: no matching app controls found.
- Automatic browser error forwarding and localhost discovery: console/network capture is real and exposed through browser tools (Area 8); automatic unsolicited forwarding and server discovery were not found.
- Generic Git-hook/PR-webhook routine triggers: real triggers are schedules, arriving files and private routine links.
- Single-turn temperature Rerun/Tune modal: actual reruns belong to comparison columns; tuning measures teammate changes.
- Live screen/file sharing and local TTS/STT call backend suite: actual voice calls use Codex/OpenAI realtime with folder/connector context and recorded words.
- Remote SSH workspace/file-sync manager and license/plan account sync: cloud computers, Claude remote control and sealed profile sync are distinct features.
- Spatial Desk board: Desk serves the full browser window; the real Board is a renderer component.
- General font-family/size/accent theme editor: actual theme/look and teammate colour controls are listed.
- General SVG artifact sandbox: real runnable HTML replies and file previews are listed.

## 5. Coverage Review

Enumerated and read every renderer component. Feature-bearing screens, panels, cards and dialogs are represented in Section 1; supporting buttons/icons/animation primitives are grouped below instead of counted as extra features.

| Coverage | Existing component files |
| :--- | :--- |
| Area 1 | `renderer/src/components/ActivityCard.tsx`, `renderer/src/components/BesideConversation.tsx`, `renderer/src/components/CancellationCard.tsx`, `renderer/src/components/ContextRing.tsx`, `renderer/src/components/CopyButton.tsx`, `renderer/src/components/DecisionCard.tsx`, `renderer/src/components/ExchangeStrip.tsx`, `renderer/src/components/ImportDialog.tsx`, `renderer/src/components/Inspector.tsx`, `renderer/src/components/JumpToBottom.tsx`, `renderer/src/components/NextSteps.tsx`, `renderer/src/components/PeerThread.tsx`, `renderer/src/components/PromptRail.tsx`, `renderer/src/components/PublishedCards.tsx`, `renderer/src/components/ResumeCard.tsx`, `renderer/src/components/SaveRecordDialog.tsx`, `renderer/src/components/SelectionAsk.tsx`, `renderer/src/components/SideChat.tsx`, `renderer/src/components/Sidebar.tsx`, `renderer/src/components/StatusChip.tsx`, `renderer/src/components/Thread.tsx`, `renderer/src/components/ThreadFind.tsx`, `renderer/src/components/ThreadItems.tsx`, `renderer/src/components/TimeMarker.tsx` |
| Area 2 | `renderer/src/components/AttachedImage.tsx`, `renderer/src/components/ChatCommands.tsx`, `renderer/src/components/Composer.tsx`, `renderer/src/components/EffortSlider.tsx`, `renderer/src/components/PagePointChips.tsx`, `renderer/src/components/PdfTile.tsx`, `renderer/src/components/ReferenceChips.tsx`, `renderer/src/components/RoutePicker.tsx`, `renderer/src/components/SentNotes.tsx` |
| Area 3 | `renderer/src/components/AgentMark.tsx`, `renderer/src/components/CliArtifacts.tsx`, `renderer/src/components/FaceLegend.tsx`, `renderer/src/components/GroupSettingsDialog.tsx`, `renderer/src/components/HandoffDivider.tsx`, `renderer/src/components/InstructionsCard.tsx`, `renderer/src/components/KeptInstructions.tsx`, `renderer/src/components/KeyCard.tsx`, `renderer/src/components/MemoryCard.tsx`, `renderer/src/components/MemoryScreen.tsx`, `renderer/src/components/NewTeammateDialog.tsx`, `renderer/src/components/PixelFace.tsx`, `renderer/src/components/RoomScreen.tsx`, `renderer/src/components/Sidebar.tsx`, `renderer/src/components/SkillsFromGitHub.tsx`, `renderer/src/components/TeamCard.tsx`, `renderer/src/components/TeamTemplates.tsx`, `renderer/src/components/TeammateBot.tsx`, `renderer/src/components/TeammateSetup.tsx`, `renderer/src/components/TuneTeammate.tsx` |
| Area 4 | `renderer/src/components/AcpAgentsSettings.tsx`, `renderer/src/components/AgentAccountsSettings.tsx`, `renderer/src/components/HandoffDivider.tsx`, `renderer/src/components/OwnModels.tsx`, `renderer/src/components/SignInButton.tsx`, `renderer/src/components/Thread.tsx` |
| Area 5 | `renderer/src/components/ApprovalCard.tsx`, `renderer/src/components/BoardScreen.tsx`, `renderer/src/components/SavedApprovalRules.tsx`, `renderer/src/components/Sidebar.tsx`, `renderer/src/components/WhatLocustCanStop.tsx` |
| Area 6 | `renderer/src/components/ChangesPanel.tsx`, `renderer/src/components/CheckFixLine.tsx`, `renderer/src/components/CommitChanges.tsx`, `renderer/src/components/DiffNotes.tsx`, `renderer/src/components/DiffView.tsx`, `renderer/src/components/EditCheckCard.tsx`, `renderer/src/components/FileTypeIcon.tsx`, `renderer/src/components/IssueList.tsx`, `renderer/src/components/LineCounts.tsx`, `renderer/src/components/PullRequestChip.tsx`, `renderer/src/components/PullRequestNewsBanner.tsx`, `renderer/src/components/ReviewChanges.tsx`, `renderer/src/components/ScoutCards.tsx`, `renderer/src/components/ScoutSetting.tsx`, `renderer/src/components/SentNotes.tsx`, `renderer/src/components/TurnUndo.tsx`, `renderer/src/components/WorktreeRow.tsx` |
| Area 7 | `renderer/src/components/OpenInTerminal.tsx`, `renderer/src/components/OwnCommand.tsx`, `renderer/src/components/TerminalDivider.tsx`, `renderer/src/components/TerminalPanel.tsx`, `renderer/src/components/TerminalToggle.tsx` |
| Area 8 | `renderer/src/components/BrowserPanel.tsx`, `renderer/src/components/BrowserToggle.tsx`, `renderer/src/components/CompareView.tsx`, `renderer/src/components/FileViewer.tsx`, `renderer/src/components/PagePointChips.tsx` |
| Area 9 | `renderer/src/components/AutomationsScreen.tsx`, `renderer/src/components/BenchmarksScreen.tsx`, `renderer/src/components/CompareBenchmark.tsx`, `renderer/src/components/CompareView.tsx`, `renderer/src/components/RoutineChanges.tsx`, `renderer/src/components/RoutineDialog.tsx`, `renderer/src/components/RoutineFileDialog.tsx`, `renderer/src/components/RoutineInputs.tsx`, `renderer/src/components/RoutineRecovery.tsx`, `renderer/src/components/RoutineTemplates.tsx` |
| Area 10 | `renderer/src/components/CallWords.tsx`, `renderer/src/components/TalkShortcutSetting.tsx`, `renderer/src/components/TalkVoice.tsx`, `renderer/src/components/VoiceButton.tsx`, `renderer/src/components/VoiceCall.tsx`, `renderer/src/components/VoicePlay.tsx`, `renderer/src/components/VoiceSettings.tsx` |
| Area 11 | `renderer/src/components/AddConnector.tsx`, `renderer/src/components/ConnectorHealth.tsx`, `renderer/src/components/FeedbackDialog.tsx`, `renderer/src/components/GitHubAccount.tsx`, `renderer/src/components/KeyboardSheet.tsx`, `renderer/src/components/KeychainNote.tsx`, `renderer/src/components/Screens.tsx`, `renderer/src/components/UsageDialog.tsx`, `renderer/src/components/UsageLine.tsx`, `renderer/src/components/UsageMeters.tsx` |
| Area 12 | `renderer/src/components/BackgroundRuns.tsx`, `renderer/src/components/ChatAppsSettings.tsx`, `renderer/src/components/CloudComputers.tsx`, `renderer/src/components/CloudSetupDialog.tsx`, `renderer/src/components/CloudTasks.tsx`, `renderer/src/components/CloudTurn.tsx`, `renderer/src/components/PhoneSettings.tsx`, `renderer/src/components/RemoteConnectCard.tsx`, `renderer/src/components/RemoteControlSetting.tsx`, `renderer/src/components/RemoteMcpSettings.tsx`, `renderer/src/components/TelegramSettings.tsx`, `renderer/src/components/YourAccountSettings.tsx` |
| Area 13 | `renderer/src/components/AwayList.tsx`, `renderer/src/components/BoardScreen.tsx`, `renderer/src/components/BootScreen.tsx`, `renderer/src/components/CommandPalette.tsx`, `renderer/src/components/FirstLaunch.tsx`, `renderer/src/components/HomeCover.tsx`, `renderer/src/components/HomeTeam.tsx`, `renderer/src/components/IdleTeammate.tsx`, `renderer/src/components/LocustMcpSettings.tsx`, `renderer/src/components/Screens.tsx`, `renderer/src/components/Sidebar.tsx`, `renderer/src/components/StarterList.tsx`, `renderer/src/components/WhatsNew.tsx` |
| Area 14 | `renderer/src/components/Bot.tsx`, `renderer/src/components/LookSetting.tsx`, `renderer/src/components/PetPicks.tsx`, `renderer/src/components/PetPuppet.tsx`, `renderer/src/components/PetSprite.tsx`, `renderer/src/components/Sidebar.tsx`, `renderer/src/components/TeammateBot.tsx`, `renderer/src/components/ThemeSetting.tsx`, `renderer/src/components/TitleBar.tsx` |
| Area 15 | `renderer/src/components/ColouredCode.tsx`, `renderer/src/components/CopyButton.tsx`, `renderer/src/components/DocPreview.tsx`, `renderer/src/components/DocumentChange.tsx`, `renderer/src/components/DocumentView.tsx`, `renderer/src/components/FileViewer.tsx`, `renderer/src/components/MathTex.tsx`, `renderer/src/components/PdfPages.tsx`, `renderer/src/components/PdfTile.tsx`, `renderer/src/components/ReplyDiagram.tsx`, `renderer/src/components/ReplyPage.tsx`, `renderer/src/components/SheetView.tsx`, `renderer/src/components/ThreadItems.tsx`, `renderer/src/components/ZoomableImage.tsx` |
| Composer/control primitives | `renderer/src/components/ArmedButton.tsx`, `renderer/src/components/ChatGlyphs.tsx`, `renderer/src/components/CommandText.tsx`, `renderer/src/components/ContextMenu.tsx`, `renderer/src/components/MetalSend.tsx`, `renderer/src/components/PlusMenu.tsx`, `renderer/src/components/Select.tsx` |
| Thread/status/navigation support | `renderer/src/components/Beam.tsx`, `renderer/src/components/ListWindow.tsx`, `renderer/src/components/OnTheComputer.tsx`, `renderer/src/components/Orb.tsx`, `renderer/src/components/RailFlyout.tsx`, `renderer/src/components/RuntimeMark.tsx`, `renderer/src/components/ThreadImage.tsx`, `renderer/src/components/WindowFallback.tsx`, `renderer/src/components/WorkingSpark.tsx` |
| Team/face/appearance support | `renderer/src/components/Icon.tsx`, `renderer/src/components/Keycaps.tsx`, `renderer/src/components/PoweredLockup.tsx` |
| Cloud/settings/form support | `renderer/src/components/ChatAppRow.tsx`, `renderer/src/components/CloudEnvironmentField.tsx`, `renderer/src/components/VoiceOrb.tsx` |

CHANGELOG: read all **295 New bullets** through 0.871.0. Added omissions include Board/away recap, prompt rail/compact, selection/side asks, published/task cards, About you/team image/interview, ACP/own models, policy, Scout fixes/comments/away, routine input/file/template/gates, comparison rubric/benchmark/tuning, voice preview, connector/report controls, phone offline/alerts, remote MCP/browser access, profile sync/recovery, startup/keep-awake, glass/rainbow, math and PowerPoint. Incremental additions are grouped in the relevant row rather than counted again; current code limits historical release claims.

## Verification

This is source inspection, not runtime certification. Read all four Rust modules, cited TS implementations, every component and the complete New-bullet inventory. Native statuses are based on code, including relevant existing tests as evidence; no tests were executed.

Path validation used Python to extract every backticked source path, resolve the documented roots, and call Path.is_file() (Path.exists() for root directories). Each evidence file was checked before writing and every final citation was checked afterward. No glob paths or proposed future filenames are cited. Protocol tags, runtime data filenames and key chords are not treated as source paths.

Checked **833 path references (393 unique), 0 missing**; all **172 component files covered**. Section 1 (recounted from feature status cells): **153 rows — 25 done, 24 partial, 104 missing**.
Checked **833 path references (393 unique), 0 missing**; all **172 component files covered**. Section 1 (recounted from feature status cells): **153 rows — 26 done, 24 partial, 103 missing**.

An independent parser checks all Section 1 feature/status rows against Section 3 per-area and grand totals, requires a gap on every partial row, and checks every component is covered. Physical source-size counts are separate from feature counts.

Not run: cargo test/build, TS tests, app launch, live Electron/MCP integration, or macOS/Windows/Linux behavior tests. Only PARITY.md was written.

M4A execution verification (2026-11-09): generic ACP synthetic runs and TypeScript ledger-reader checks are recorded in M4A-NOTES.md. The audit-only verification above is historical; no real turn or window was launched for M4A.

M4C execution verification (2026-11-09): Cursor, Copilot, Antigravity and Muse native drivers and synthetic ledgers are recorded in M4C-NOTES.md. No window or live one-line CLI turn was launched for M4C. The composite named-runtime row stays **partial** (no discovery/sign-in UI; live CLI turns unrun).

P5 verification (2026-11-10): partial implementation, not a closed parity round. See P5-NOTES.md for tests, screenshots and remaining behavior. No row was promoted to done by P5.

P6 evidence: `P6-NOTES.md`. Area 3 gains three partial rows; no composite row is promoted to done without the remaining Electron behavior/tests. Native turns now receive the native copy of standing instructions, About you, and applicable kept memory. Recent menus use an inline hover/selected ⋯ button with Electron’s accessible name.

P7 verification (2026-11-10): partial Changes/diff/Undo implementation, with unported TS assertions and storage/UI gaps recorded in `P7-NOTES.md`. Three final release runs each passed 360 tests (13 existing live tests ignored); release build zero warnings; clippy no new-module findings; both synthetic CLI scripts pass. Dark/light Changes and Undo scenes rendered and inspected at 900×900, scale 1. No live window or real turn launched. Area 6 is 0 done / 3 partial / 10 missing; M5 remains unreached.

P7 review (2026-11-10): Undo atomic save + mode-preserving restore + skip-unreadable snapshot; empty-diff words; panel closes off non-repo; git stdout on non-zero; untracked symlink listing; completeness hunk/line words; green/red count marks; gutter/sign colours; per-fold context expand. Review section in `P7-NOTES.md`. Three release runs each 366 passed (13 ignored); release build zero warnings; clippy existing findings only; both CLI scripts pass; snaps re-inspected under `/Users/locust/work/scratch/p7-snaps/`. Live window/exit and real Undo still unrun. Area 6 remains 0 done / 3 partial / 10 missing; M5 unreached.
P8 evidence: `P8-NOTES.md`. Area 5: saved approval rules **done**; command reach/plain/guard, workspace attach/settings, and shared secrets **partial**. No window or live turn was launched for P8.

P9 verification: see `P9-NOTES.md`. Current Section 1 recount: **153 rows — 26 done, 30 partial, 97 missing**; Area 7 is 0 done / 3 partial / 0 missing. This is an incomplete parity implementation, not M5 reached. Earlier audit counts above are historical.

P9 final checks (2026-11-10): 410 release tests passed ×3 (13 existing live tests ignored); release build zero warnings; clippy four existing argument-count findings only; M1/M3a synthetic CLI scripts pass. Final dark/light terminal scenes at 900×900 scale 1 rendered and inspected. No window or real agent turn. Remaining gaps and TS test disposition: `P9-NOTES.md`.
