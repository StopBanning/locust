# Locust Desktop → Native Rust (`locust-native`) 1-to-1 Parity Checklist

## Overview & Baseline

- **TypeScript Desktop App (`/Users/locust/work/locust`)**: Electron + React/TypeScript across `apps/desktop/src/{renderer,main,shared,preload}` and `packages/{runtime-adapters,mission-store,cli}`, through `CHANGELOG.md` release `0.871.0`.
  - **Non-test TypeScript/TSX source**: **736 files, 205,713 lines** (`renderer`: 314 files / 85,401 lines; `shared`: 128 files / 24,998 lines; `preload`: 1 file / 38 lines; `main`: 253 files / 72,836 lines; `packages/runtime-adapters`: 32 files / 18,212 lines; `packages/mission-store`: 4 files / 3,735 lines; `packages/cli`: 4 files / 493 lines).
- **Native Rust App (`/Users/locust/work/locust-wt/native/apps/native`)**: Built on `rune-app` / `rune-kit` / `rune-surface`.
  - **Non-test Rust source**: **4 files, 4,329 lines** (6,471 total lines including `#[cfg(test)]` modules):
    - `src/main.rs`: 2,143 non-test lines (2,706 total) — sidebar, thread view, composer, model picker, carry-on card, dark/light palette, CLI `--ask`, `--snap`, `--bench`.
    - `src/quick.rs`: 1,150 non-test lines (1,806 total) — macOS floating Quick Ask panel (`Option+Space`), teammate cycling, multiline `Shift+Enter`, background ledger tailing.
    - `src/ledger.rs`: 715 non-test lines (1,375 total) — read-only JSONL reader for `<userData>/mission-ledger/*.jsonl` (schema v1..=27, continuation chains, delta streaming, tool/step/plan/note/carry-on synthesis).
    - `src/engine.rs`: 321 non-test lines (584 total) — HTTP bearer-token client for Electron's local MCP server (`locust-mcp-connection.json`, calling `list_teammates`, `list_conversations`, `start_conversation`, `send_message`, `list_models`, `stop_turn`, `carry_on`, `read_reply`).
- **Today's Architecture**: `locust-native` reads mission ledger `.jsonl` files directly from disk for live transcript rendering and conversation listing, and delegates all mutations and queries (starting turns, sending messages, stopping turns, carry-on, listing teammates/models) to the running Electron app over its localhost MCP HTTP endpoint.

---

## 1. Feature Parity by Area (15 Areas)

Status definitions:
- **done**: Implemented end-to-end in `locust-native` (either standalone or via the MCP client/ledger reader with equivalent UX).
- **partial**: Core path works in `locust-native`, but sub-features, controls, or standalone engine support are incomplete.
- **missing**: Not yet implemented in `locust-native`.
- **depends on engine**: `yes` if the feature requires main-process state, subprocesses, filesystem/git mutation, OS integration, or runtime execution; `no` if it is pure UI/rendering on top of already-loaded data.

### Area 1: Conversations & Thread Rendering
*Non-test TS lines: **60,163** (`renderer + shared`: 39,001 across 84 files; `main + packages`: 21,162 across 11 files) | Native Rust: ~1,550 lines across `src/main.rs` and `src/ledger.rs`*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Sidebar recent conversations list with status dots & relative age | `renderer/src/components/Sidebar.tsx`, `main/mission-history.ts` | **partial** | yes |
| Direct local `.jsonl` mission ledger reader & live tailing (v1..=27) | `packages/mission-store/src/mission-ledger.ts`, `main/codex-mission.ts` | **done** | yes |
| Continuation chain splicing (`continuesFrom.missionId`) | `packages/mission-store/src/mission-ledger.ts`, `renderer/src/components/Thread.tsx` | **done** | yes |
| Streaming assistant Markdown rendering & user message bubbles | `renderer/src/components/Thread.tsx`, `renderer/src/components/RichBlock.tsx` | **partial** | no |
| Live activity status bar (`Working…`, `WaitingForYou`, `Ready`, error) | `renderer/src/components/Thread.tsx`, `renderer/src/mission-activity.ts` | **done** | no |
| Tool call rows (status icon, summary, expand/collapse output & args) | `renderer/src/components/Thread.tsx`, `renderer/src/tool-Summary.ts` | **partial** | no |
| Step & plan checklist rendering (`step.*`, `plan.updated`) | `renderer/src/components/Thread.tsx` | **partial** | no |
| Adapter diagnostic warning/error notes & rate-limit banners | `renderer/src/components/Thread.tsx`, `shared/error-Helpful.ts` | **partial** | no |
| Multi-pane split threads (side-by-side conversations, pane focus) | `renderer/src/App.tsx`, `renderer/src/pane-layout.ts` | **missing** | no |
| Conversation tabs, pinning, archiving, folders/projects, rename, delete | `renderer/src/components/Sidebar.tsx`, `main/mission-history.ts` | **missing** | yes |
| In-thread search (`Cmd+F`) and cross-conversation full-text search | `renderer/src/components/ThreadSearchBar.tsx`, `renderer/src/conversationSearch.ts` | **missing** | yes |
| Message actions (copy, edit & resend, fork/branch from turn, bookmark/pin) | `renderer/src/components/Thread.tsx`, `main/codex-mission.ts` | **missing** | yes |
| Subagent / background agent nested cards (`parentItemId` tree & transcript) | `renderer/src/components/SubagentCard.tsx`, `renderer/src/subagent-tree.ts` | **missing** | yes |
| Peer teammate share cards (`mission.peer` / `<locust-share>` handoffs) | `renderer/src/components/PeerShareCard.tsx`, `main/peer-share-relay.ts` | **missing** | yes |
| Special reply blocks (`<locust-ask>` option buttons, `<locust-file>`, `<locust-next>`) | `renderer/src/locust-blocks.ts`, `renderer/src/components/Thread.tsx` | **missing** | yes |
| Reasoning / thinking collapsible trace & live elapsed timer | `renderer/src/components/Thread.tsx` | **missing** | no |
| Per-turn token usage, context window meter, cost badge & compaction notice | `renderer/src/components/ContextMeter.tsx`, `renderer/src/usage-cost.ts` | **missing** | yes |
| Conversation export (Markdown, JSON, HTML, shareable link/bundle) | `main/mission-export.ts`, `renderer/src/components/Thread.tsx` | **missing** | yes |
| Session import from external CLI histories (Claude Code, Codex, Cursor, etc.) | `main/cli-session-import.ts`, `renderer/src/components/ImportModal.tsx` | **missing** | yes |
| Read-aloud / TTS playback on assistant messages | `renderer/src/speech-synthesis.ts`, `renderer/src/components/Thread.tsx` | **missing** | no |

### Area 2: Composer & Input
*Non-test TS lines: **5,683** (`renderer + shared`: 4,994 across 24 files; `main`: 689 across 1 file) | Native Rust: ~420 lines in `src/main.rs`*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Text input, `Send` button, `Stop` button (ownership-aware `stop_turn`) | `renderer/src/components/Composer.tsx`, `main/locust-mcp-tools.ts` | **done** | yes |
| Unsent draft restoration on failed or cancelled send | `renderer/src/components/Composer.tsx`, `renderer/src/draft-store.ts` | **done** | no |
| Per-turn model & effort chip selector in composer bar | `renderer/src/components/Composer.tsx`, `renderer/src/components/ModelPicker.tsx` | **done** | yes |
| Multiline auto-expanding editor (`Shift+Enter` newline in main window) | `renderer/src/components/Composer.tsx` | **partial** | no |
| Persistent per-conversation & per-teammate drafts across restarts | `renderer/src/draft-store.ts` | **missing** | no |
| `@` file, folder, symbol, and `@teammate` autocomplete menu | `renderer/src/components/MentionMenu.tsx`, `main/workspace-files.ts` | **missing** | yes |
| `/` slash command & skill picker menu in composer | `renderer/src/components/SlashCommandMenu.tsx`, `shared/slash-commands.ts` | **missing** | yes |
| Image, screenshot, and file attachment drag-and-drop / paste | `renderer/src/components/Composer.tsx`, `renderer/src/attachment-utils.ts` | **missing** | yes |
| Follow-up message queue while teammate is busy (queue, steer now, edit queued) | `renderer/src/components/QueuedMessagesBar.tsx`, `main/codex-mission.ts` | **missing** | yes |
| Prompt templates, saved snippets, and Up-arrow input history | `renderer/src/components/Composer.tsx`, `renderer/src/prompt-history.ts` | **missing** | no |
| Reference folders attachment bar (read-only context folders per turn/mission) | `renderer/src/components/ReferenceFoldersBar.tsx`, `main/codex-mission.ts` | **missing** | yes |
| Mode toggles in composer (Plan mode / Ask / Auto-edit / Sandbox level) | `renderer/src/components/Composer.tsx`, `shared/mission-modes.ts` | **missing** | yes |

### Area 3: Teammates, Rooms, Memory & Skills
*Non-test TS lines: **17,084** (`renderer + shared`: 8,288 across 33 files; `main + packages`: 8,796 across 20 files) | Native Rust: ~260 lines in `src/main.rs` & `src/engine.rs`*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Sidebar teammate list (avatar initial, name, busy dot, role, runtime · model) | `renderer/src/components/Sidebar.tsx`, `main/teammate-store.ts` | **done** | yes |
| Select teammate (`Cmd+1..9` or click) & start fresh conversation (`Cmd+N`) | `renderer/src/App.tsx`, `renderer/src/components/Sidebar.tsx` | **done** | yes |
| Teammate creation, editing, role/instructions, default runtime/model, deletion | `renderer/src/components/TeammateModal.tsx`, `main/teammate-store.ts` | **missing** | yes |
| Built-in starter team presets (Argon, Codec, Scout, Sage, etc.) | `shared/teammate-presets.ts`, `main/teammate-store.ts` | **missing** | yes |
| Multi-teammate Rooms & Group conversations (`@mention` routing, room turn loop) | `renderer/src/components/RoomView.tsx`, `main/room-store.ts`, `main/group-store.ts` | **missing** | yes |
| Cross-teammate `<locust-share to="...">` automatic relay & handoff execution | `main/peer-share-relay.ts`, `main/codex-mission.ts` | **missing** | yes |
| Shared team memory store (`<locust-memory>` parsing, `remember`, `forget`, UI) | `main/memory-store.ts`, `renderer/src/components/MemoryPanel.tsx` | **missing** | yes |
| Skills & instructions manager (`SKILL.md`, `CLAUDE.md`, `AGENTS.md` discovery/UI) | `main/skill-store.ts`, `renderer/src/components/SkillsScreen.tsx` | **missing** | yes |
| Teammate SSH / GPG signing keys & workspace isolation settings | `main/teammate-keys.ts`, `main/teammate-workspace.ts` | **missing** | yes |
| Custom teammate avatars, badges, and status indicators | `renderer/src/components/TeammateAvatar.tsx`, `shared/teammate-types.ts` | **missing** | no |

### Area 4: Runtimes, Routing & Fallback
*Non-test TS lines: **30,570** (`renderer + shared`: 4,696 across 33 files; `main + packages`: 25,874 across 67 files) | Native Rust: ~380 lines in `src/main.rs`, `src/engine.rs`, `src/ledger.rs`*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Carry-on banner on rate limit / failure with runtime & model picker (`carry_on`) | `renderer/src/components/CarryOnCard.tsx`, `main/carry-on.ts` | **done** | yes |
| Query available models & reasoning effort levels across runtimes (`list_models`) | `main/model-catalog.ts`, `main/locust-mcp-tools.ts` | **done** | yes |
| Codex CLI / app-server runtime adapter (`@teammate/runtime-adapters`) | `packages/runtime-adapters/src/codex-adapter.ts`, `main/codex-mission.ts` | **missing** | yes |
| Claude Code runtime adapter | `packages/runtime-adapters/src/claude-code-adapter.ts` | **missing** | yes |
| Gemini CLI & Antigravity runtime adapters | `packages/runtime-adapters/src/gemini-adapter.ts`, `main/antigravity-mission.ts` | **missing** | yes |
| OpenCode, Cursor, Aider, Goose, Amp, Copilot, Kiro, Qwen, Droid, Windsurf adapters | `packages/runtime-adapters/src/*-adapter.ts` (20+ adapters) | **missing** | yes |
| Direct API / Local LLM adapters (Ollama, LM Studio, OpenAI/Anthropic/OpenRouter) | `packages/runtime-adapters/src/local-llm-adapter.ts`, `main/local-models.ts` | **missing** | yes |
| Automatic smart routing (task classification, model tiering, fallback chains) | `main/router-service.ts`, `shared/routing-policy.ts` | **missing** | yes |
| Multi-account pool & automatic account rotation on quota/rate limit | `main/runtime-accounts.ts`, `renderer/src/components/AccountsModal.tsx` | **missing** | yes |
| Runtime detection, health check, CLI install/login wizard & version probing | `main/runtime-doctor.ts`, `renderer/src/components/RuntimeSetup.tsx` | **missing** | yes |
| Mid-conversation handoff / context compaction packet across different runtimes | `main/handoff-packet.ts`, `main/carry-on.ts` | **partial** | yes |

### Area 5: Approvals & Permissions
*Non-test TS lines: **7,716** (`renderer + shared`: 4,610 across 20 files; `main`: 3,106 across 17 files) | Native Rust: 0 lines*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Inline approval request card in thread (Allow once, Allow for session, Deny) | `renderer/src/components/ApprovalCard.tsx`, `main/approval-channel.ts` | **missing** | yes |
| Shell command safety analysis & dangerous command guard | `main/command-guard.ts`, `shared/command-safety.ts` | **missing** | yes |
| Path boundary & outside-workspace write/read permission checks | `main/permission-host.ts`, `shared/permission-rules.ts` | **missing** | yes |
| Network / domain access approval prompts & allowlists | `main/permission-host.ts`, `renderer/src/components/ApprovalCard.tsx` | **missing** | yes |
| Saved permission rules manager (per-workspace & global allow/deny rules) | `main/approval-rules-store.ts`, `renderer/src/components/PermissionsSettings.tsx` | **missing** | yes |
| Autonomy / sandbox mode presets (Supervised, Auto-edit, Full auto, Read-only) | `shared/mission-modes.ts`, `main/codex-mission.ts` | **missing** | yes |
| Secret & credential leak scanner on outbound prompts and tool outputs | `main/secret-guard.ts`, `shared/secret-patterns.ts` | **missing** | yes |
| Pending approvals badge in sidebar, dock/tray alert, and Quick Ask forwarding | `renderer/src/components/Sidebar.tsx`, `main/approval-channel.ts` | **missing** | yes |

### Area 6: Changes Panel, Diffs, Git, Worktrees, PRs & Scout
*Non-test TS lines: **12,431** (`renderer + shared`: 6,345 across 33 files; `main`: 6,086 across 29 files) | Native Rust: 0 lines*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Right-hand Changes panel (modified/added/deleted files per turn & working tree) | `renderer/src/components/ChangesPanel.tsx`, `main/git-status-service.ts` | **missing** | yes |
| Unified & split syntax-highlighted diff viewer with inline stage/revert hunks | `renderer/src/components/DiffViewer.tsx`, `renderer/src/diff-parser.ts` | **missing** | yes |
| Turn checkpoints & one-click Undo / Restore workspace to before turn | `main/checkpoint-service.ts`, `renderer/src/components/Thread.tsx` | **missing** | yes |
| Git branch switcher, commit composer, AI commit message generator, push/pull | `renderer/src/components/GitPanel.tsx`, `main/git-service.ts` | **missing** | yes |
| Automatic Git worktree / isolated branch per mission (`worktree-manager.ts`) | `main/worktree-manager.ts`, `renderer/src/components/WorktreeBar.tsx` | **missing** | yes |
| Worktree merge-back, conflict resolution helper, and cleanup | `main/worktree-manager.ts`, `renderer/src/components/MergeModal.tsx` | **missing** | yes |
| GitHub / GitLab Pull Request creation, status checks, review comments & merge | `main/pr-service.ts`, `renderer/src/components/PullRequestPanel.tsx` | **missing** | yes |
| Issue tracker integration (GitHub Issues / Linear browse & start mission from issue) | `main/issue-service.ts`, `renderer/src/components/IssuesModal.tsx` | **missing** | yes |
| Post-turn Edit Check (`mission.edit_check`: auto-run lint/typecheck/tests on changed files) | `main/edit-check-service.ts`, `renderer/src/components/EditCheckCard.tsx` | **missing** | yes |
| Scout automated code review on diffs/PRs with inline findings cards | `main/scout-review.ts`, `renderer/src/components/ScoutReviewPanel.tsx` | **missing** | yes |

### Area 7: Terminal & Own Commands
*Non-test TS lines: **2,498** (`renderer + shared`: 762 across 11 files; `main`: 1,736 across 9 files) | Native Rust: 0 lines*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Integrated interactive PTY terminal drawer (multi-tab, resize, ANSI/xterm) | `renderer/src/components/TerminalDrawer.tsx`, `main/terminal-host.ts` | **missing** | yes |
| "Run your own command" inline in thread (`!` prefix or command bar) | `main/own-command-host.ts`, `renderer/src/components/Thread.tsx` | **missing** | yes |
| Attach terminal selection / failed command output directly to composer | `renderer/src/components/TerminalDrawer.tsx`, `renderer/src/components/Composer.tsx` | **missing** | no |
| Background dev-server / daemon process manager (ports, logs, stop/restart) | `main/process-manager.ts`, `renderer/src/components/ProcessesBar.tsx` | **missing** | yes |

### Area 8: Built-in Browser & Page Previews
*Non-test TS lines: **4,052** (`renderer + shared`: 1,552 across 8 files; `main`: 2,500 across 8 files) | Native Rust: 0 lines*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Built-in web preview pane with URL bar, navigation, reload, and responsive viewport | `renderer/src/components/BrowserPane.tsx`, `main/browser-host.ts` | **missing** | yes |
| Localhost dev server auto-detection & live preview opening | `main/page-preview-server.ts`, `renderer/src/components/BrowserPane.tsx` | **missing** | yes |
| Visual element picker ("Click to pick element" → DOM/CSS/React component to prompt) | `main/browser-pick-bridge.ts`, `renderer/src/components/BrowserPane.tsx` | **missing** | yes |
| Browser console error & network failure capture forwarded to teammate | `main/browser-host.ts`, `renderer/src/components/BrowserPane.tsx` | **missing** | yes |
| Headless browser automation tools exposed to teammates (screenshot, click, read) | `main/browser-mcp-tools.ts`, `main/browser-host.ts` | **missing** | yes |

### Area 9: Routines, Compare & Benchmarks
*Non-test TS lines: **10,472** (`renderer + shared`: 5,862 across 27 files; `main`: 4,610 across 17 files) | Native Rust: ~160 lines (`--bench` in `src/main.rs`)*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Scheduled Routines (cron/interval jobs, wake behavior, run history & logs) | `main/routine-runner.ts`, `main/routine-store.ts`, `renderer/src/components/RoutinesScreen.tsx` | **missing** | yes |
| Event-triggered Routines (git hooks, file watch triggers, PR/webhook triggers) | `main/routine-file-watch.ts`, `main/routine-hooks.ts` | **missing** | yes |
| Side-by-side Compare mode (send same prompt to 2–4 teammates/models in isolated worktrees) | `main/compare-runner.ts`, `renderer/src/components/CompareScreen.tsx` | **missing** | yes |
| AI Judge / rubric scoring & pick-winner merge in Compare mode | `main/compare-judge.ts`, `renderer/src/components/CompareScreen.tsx` | **missing** | yes |
| Turn Reruns & Tune (re-run turn with tweaked instructions/temperature/model) | `main/rerun-service.ts`, `renderer/src/components/RerunModal.tsx` | **missing** | yes |
| UI frame-time & memory benchmark harness (`--bench <out.json>`) | `apps/native/src/main.rs` (native); `main/perf-diagnostics.ts` (TS) | **done** | no |

### Area 10: Calls, Voice Typing & Quick Ask
*Non-test TS lines: **4,293** (`renderer + shared`: 2,919 across 23 files; `main`: 1,374 across 10 files) | Native Rust: 1,150 lines in `src/quick.rs`*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Global Quick Ask floating panel (`Option+Space`, teammate Tab cycle, streaming reply) | `main/quick-ask-window.ts`, `renderer/src/components/QuickAsk.tsx` | **done** | yes |
| Quick Ask background session tracking & `WaitingForYou` indicator while hidden | `apps/native/src/quick.rs`, `main/quick-ask-window.ts` | **done** | yes |
| Quick Ask "Open in main window", screenshot/selection context capture | `main/quick-ask-window.ts`, `renderer/src/components/QuickAsk.tsx` | **missing** | yes |
| Push-to-talk voice dictation in composer & global talk shortcut | `main/voice-transcription.ts`, `renderer/src/components/VoiceButton.tsx` | **missing** | yes |
| Live bidirectional Voice Call with teammate (Codex/OpenAI Realtime & local TTS/STT) | `main/codex-voice-call.ts`, `renderer/src/components/CallOverlay.tsx` | **missing** | yes |
| Call transcript logging into mission ledger & live screen/file sharing during call | `main/codex-voice-call.ts`, `renderer/src/components/CallOverlay.tsx` | **missing** | yes |

### Area 11: Settings, Usage, Privacy & Backup
*Non-test TS lines: **10,930** (`renderer + shared`: 7,337 across 33 files; `main`: 3,593 across 16 files) | Native Rust: 0 lines*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Settings modal/screen with categorized navigation (`settingsPages.ts`) | `renderer/src/components/SettingsModal.tsx`, `renderer/src/settingsPages.ts` | **missing** | yes |
| General, Appearance, Notifications, Sound effects & Keyboard shortcuts customization | `renderer/src/components/SettingsModal.tsx`, `main/settings-store.ts` | **missing** | yes |
| Usage & cost analytics dashboard (daily/weekly spend, tokens by model & teammate, budgets) | `renderer/src/components/UsageScreen.tsx`, `main/usage-ledger.ts` | **missing** | yes |
| OS Keychain / `safeStorage` API key & secret vault management | `main/keychain-service.ts`, `renderer/src/components/SecretsSettings.tsx` | **missing** | yes |
| Privacy controls, local data retention policy, and redaction settings | `main/privacy-service.ts`, `renderer/src/components/PrivacySettings.tsx` | **missing** | yes |
| Workspace & app state backup, export, and restore archive | `main/backup-service.ts`, `renderer/src/components/BackupSettings.tsx` | **missing** | yes |
| External MCP server client configuration (add/enable stdio & HTTP MCP servers) | `main/mcp-client-manager.ts`, `renderer/src/components/McpSettings.tsx` | **missing** | yes |

### Area 12: Cloud, Remote, Phone, Chat Apps & Desk
*Non-test TS lines: **15,577** (`renderer + shared`: 5,849 across 23 files; `main`: 9,728 across 35 files) | Native Rust: 0 lines*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Locust Anywhere / Phone companion web & mobile pairing (QR code, E2E relay/tunnel) | `main/anywhere-server.ts`, `renderer/src/components/AnywhereModal.tsx` | **missing** | yes |
| Cloud computers & remote sandbox environments (provision, sync, run turns remotely) | `main/cloud-computers.ts`, `main/cloud-tasks.ts`, `renderer/src/components/CloudScreen.tsx` | **missing** | yes |
| Claude Cloud / background remote runs & status syncing | `main/claude-cloud.ts`, `main/background-runs.ts` | **missing** | yes |
| Chat bridges: Telegram, Discord, and Slack bots connected to teammates | `main/telegram-bridge.ts`, `main/discord-bridge.ts`, `main/slack-bridge.ts` | **missing** | yes |
| Desk view (spatial status board of active teammates, tasks, and queues) | `renderer/src/components/DeskScreen.tsx`, `main/desk-service.ts` | **missing** | yes |
| Remote SSH / remote workspace connection & file synchronization | `main/remote-workspace.ts`, `renderer/src/components/RemoteModal.tsx` | **missing** | yes |
| Account sign-in, license/plan sync, and cloud settings sync | `main/account-sync.ts`, `renderer/src/components/AccountSettings.tsx` | **missing** | yes |

### Area 13: Onboarding, Home, Updates & CLI
*Non-test TS lines: **7,440** (`renderer + shared`: 3,876 across 27 files; `main + packages`: 3,564 across 21 files) | Native Rust: ~310 lines in `src/main.rs` & `src/engine.rs`*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| CLI headless ask mode (`--ask <teammate> <message>`) & `@teammate/cli` | `apps/native/src/main.rs`, `packages/cli/src/index.ts` | **done** | yes |
| Automated visual snapshot testing (`--snap <path> --scene <name>`) | `apps/native/src/main.rs` (14 scenes across main & quick) | **done** | no |
| Local MCP server door (`locust-mcp-connection.json`, HTTP `/tools/call`) | `main/locust-mcp-server.ts`, `main/locust-mcp-tools.ts`, `apps/native/src/engine.rs` | **partial** | yes |
| First-run onboarding wizard (runtime detection, sign-in, starter team, first mission) | `renderer/src/components/OnboardingWizard.tsx`, `main/onboarding-service.ts` | **missing** | yes |
| Workspace / project switcher (open folder, recent workspaces, multi-window) | `renderer/src/components/WorkspacePicker.tsx`, `main/workspace-store.ts` | **missing** | yes |
| Home screen (daily briefing, suggested follow-ups, quick actions, active routines) | `renderer/src/components/HomeScreen.tsx`, `main/home-briefing.ts` | **missing** | yes |
| Auto-updater (check, download, release notes banner, restart to update) | `main/updater-service.ts`, `renderer/src/components/UpdateBanner.tsx` | **missing** | yes |
| Command palette (`Cmd+K`) for global navigation and actions | `renderer/src/components/CommandPalette.tsx`, `shared/commands.ts` | **missing** | no |

### Area 14: Pets, Bots & Look / Window Chrome
*Non-test TS lines: **12,857** (`renderer + shared`: 11,700 across 50 files; `main`: 1,157 across 4 files) | Native Rust: ~250 lines in `src/main.rs`*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| System Dark & Light palette switching (`cx.system_dark()`) | `apps/native/src/main.rs`, `renderer/src/theme.ts` | **done** | no |
| Custom themes, accent colors, font family/size, and macOS vibrancy/glass | `renderer/src/theme.ts`, `main/window-glass.ts` | **missing** | yes |
| Animated desktop/sidebar Pets & Bots companions (idle, thinking, celebrating states) | `renderer/src/components/PetCanvas.tsx`, `renderer/src/pets/*`, `main/pet-library.ts` | **missing** | yes |
| Menu bar tray icon with live teammate status, quick actions, and pending approvals | `main/tray-service.ts` | **missing** | yes |
| Custom window titlebar chrome, traffic light alignment, and collapsible sidebars | `renderer/src/components/Titlebar.tsx`, `main/window-chrome.ts` | **partial** | no |

### Area 15: PDFs, Diagrams, Office Docs & File Viewer
*Non-test TS lines: **3,947** (`renderer + shared`: 2,646 across 34 files; `main`: 1,301 across 8 files) | Native Rust: 0 lines*

| Feature | Where in TS (main files) | Native Status | Depends on Engine |
| :--- | :--- | :---: | :---: |
| Syntax-highlighted code blocks with copy button, file path header & line numbers | `renderer/src/components/CodeBlock.tsx`, `apps/native/src/main.rs` | **partial** | no |
| Clickable `file://` links & workspace file preview drawer (read file with line range) | `renderer/src/components/FilePreviewDrawer.tsx`, `main/file-viewer-service.ts` | **missing** | yes |
| Inline Mermaid diagram & flowchart renderer with zoom/export | `renderer/src/components/MermaidBlock.tsx` | **missing** | no |
| Built-in PDF viewer (page navigation, text extraction, cite page to prompt) | `renderer/src/components/PdfViewer.tsx`, `main/pdf-service.ts` | **missing** | yes |
| DOCX, XLSX spreadsheet, CSV table, and image lightbox viewers | `renderer/src/components/SpreadsheetViewer.tsx`, `main/office-doc-service.ts` | **missing** | yes |
| Interactive HTML / SVG artifact sandbox preview in thread | `renderer/src/components/ArtifactPreview.tsx` | **missing** | no |

---

## 2. Engine Breakdown: Main-Process Services & Standalone Rust Requirements

Today, `locust-native` depends on the Electron main process for every write path and runtime interaction via `http://127.0.0.1:<port>/tools/call` (`locust-mcp-connection.json`). To run **standalone without Electron**, `locust-native` must replace the following TypeScript engine services (`apps/desktop/src/main` + `packages/*`, totaling **95,276 non-test lines** across 293 files):

| Engine Service / Subsystem | Primary TS Files (`main/` & `packages/`) | TS Non-Test Lines | Used by UI Areas | What Native Rust Needs to Run Without Electron |
| :--- | :--- | ---: | :--- | :--- |
| **1. Runtime Adapters (`@teammate/runtime-adapters`)** | `packages/runtime-adapters/src/*.ts` (Codex, Claude Code, Gemini, OpenCode, Cursor, Aider, Goose, Amp, Local LLM, etc.) | 18,212 | Areas 1, 4, 9, 10 | Native async subprocess & stdio/JSON-RPC/HTTP drivers (`tokio::process`) spawning CLI runtimes (`codex`, `claude`, `gemini`, `opencode`, etc.) and direct HTTP streaming for local/API models, emitting normalized `MissionEvent` streams. |
| **2. Mission Store & Ledger Writer (`@teammate/mission-store` + `mission-history`)** | `packages/mission-store/src/*.ts`, `main/mission-history.ts`, `main/mission-export.ts` | 6,418 | Areas 1, 2, 4, 9 | Extend `src/ledger.rs` from read-only to an atomic append-only JSONL writer (schema v27), index/cache manager, continuation linker, and conversation search/archive/export service. |
| **3. Mission Orchestrator (`codex-mission`, `antigravity-mission`, `carry-on`)** | `main/codex-mission.ts`, `main/antigravity-mission.ts`, `main/carry-on.ts`, `main/handoff-packet.ts` | 19,845 | Areas 1, 2, 3, 4, 5 | Turn lifecycle state machine: prompt assembly (system prompt, skills, memories, reference folders, `<locust-*>` instructions), streaming delta aggregation, tool tracking, rate-limit detection, follow-up queue draining, and carry-on handoff packet generation. |
| **4. Routing, Model Catalog & Multi-Account Pool** | `main/router-service.ts`, `main/model-catalog.ts`, `main/runtime-accounts.ts`, `main/runtime-doctor.ts`, `main/local-models.ts` | 5,230 | Areas 2, 4, 11, 13 | Native model registry, CLI availability/auth prober, prompt complexity router, fallback policy evaluator, and credential/profile switcher across multiple runtime accounts. |
| **5. Teammate, Room, Memory & Skill Stores + Peer Relay** | `main/teammate-store.ts`, `main/room-store.ts`, `main/group-store.ts`, `main/memory-store.ts`, `main/skill-store.ts`, `main/peer-share-relay.ts`, `main/teammate-keys.ts` | 8,796 | Areas 1, 2, 3, 13 | JSON/SQLite persistence in `<userData>` for teammates, rooms, and memories; parser for `<locust-memory>`, `<locust-share>`, `<locust-ask>`, `<locust-file>`, `<locust-next>`; automatic peer-turn dispatcher; `SKILL.md` / `CLAUDE.md` / `AGENTS.md` loader. |
| **6. Approvals, Permissions, Command Guard & Secret Scanner** | `main/approval-channel.ts`, `main/permission-host.ts`, `main/command-guard.ts`, `main/approval-rules-store.ts`, `main/secret-guard.ts` | 3,106 | Areas 1, 5, 7, 10 | Shell AST/regex safety analyzer, workspace path boundary validator, secret pattern detector, persistent allow/deny rule store, and oneshot async channel pausing adapter tool execution until the user clicks Allow/Deny in UI. |
| **7. Git, Worktrees, Checkpoints, Diffs, PRs, Issues & Scout** | `main/git-service.ts`, `main/git-status-service.ts`, `main/worktree-manager.ts`, `main/checkpoint-service.ts`, `main/pr-service.ts`, `main/issue-service.ts`, `main/edit-check-service.ts`, `main/scout-review.ts` | 6,086 | Areas 1, 6, 9 | Git CLI / `git2` wrapper for status, unified diffs, hunk staging, pre-turn snapshot refs (checkpoints/undo), per-mission git worktree creation/merge, `gh`/`glab` or REST client for PRs/issues, post-turn lint/test runner (`mission.edit_check`), and Scout diff review runner. |
| **8. Routines, File Watch, Compare & Reruns** | `main/routine-runner.ts`, `main/routine-store.ts`, `main/routine-file-watch.ts`, `main/routine-hooks.ts`, `main/compare-runner.ts`, `main/compare-judge.ts`, `main/rerun-service.ts` | 4,610 | Area 9 | Cron/interval scheduler (`tokio-cron-scheduler` / timer loop), `notify` filesystem watcher, parallel worktree mission fan-out for Compare mode, rubric judge prompt runner, and turn rerun cloner. |
| **9. Terminal PTY Host, Own Commands & Process Manager** | `main/terminal-host.ts`, `main/own-command-host.ts`, `main/process-manager.ts` | 1,736 | Area 7 | Native PTY manager (`portable-pty` / `libc::forkpty`), ANSI terminal state machine (`vte` / `alacritty_terminal`), inline user shell command runner writing to ledger, and background dev-server tracker. |
| **10. Built-in Browser Host, Preview Server & Element Picker** | `main/browser-host.ts`, `main/page-preview-server.ts`, `main/browser-pick-bridge.ts`, `main/browser-mcp-tools.ts` | 2,500 | Area 8 | Native `WKWebView` surface (via `rune-surface` / `objc2`), injected JS bridge for DOM element picking & console/network forwarding, and headless automation endpoints for teammates. |
| **11. Voice Transcription, Realtime Voice Call & Quick Ask Host** | `main/voice-transcription.ts`, `main/codex-voice-call.ts`, `main/quick-ask-window.ts` | 1,374 | Area 10 | Native audio capture (`CoreAudio` / `cpal`), local Whisper or cloud STT client, WebRTC/WebSocket realtime audio session for voice calls (Quick Ask window host is already native in `src/quick.rs`). |
| **12. Settings, Usage Ledger, Keychain, Privacy, Backup & MCP Client** | `main/settings-store.ts`, `main/usage-ledger.ts`, `main/keychain-service.ts`, `main/privacy-service.ts`, `main/backup-service.ts`, `main/mcp-client-manager.ts` | 3,593 | Areas 1, 11 | macOS Security framework Keychain wrapper (`security-framework`), settings JSON store with live file watch, token/cost rollup aggregator, zip backup/restore, and MCP client manager for external stdio/SSE tools. |
| **13. Cloud, Anywhere/Phone, Chat Bridges & Desk** | `main/anywhere-server.ts`, `main/cloud-computers.ts`, `main/cloud-tasks.ts`, `main/claude-cloud.ts`, `main/telegram-bridge.ts`, `main/discord-bridge.ts`, `main/slack-bridge.ts`, `main/desk-service.ts`, `main/account-sync.ts` | 9,728 | Area 12 | Embedded HTTP/WebSocket server + relay client for Locust Anywhere mobile access, REST/WebSocket clients for Cloud Computers, Telegram/Discord/Slack bot adapters, and account sync client. |
| **14. Local MCP Server Door, CLI, Onboarding & Updater** | `main/index.ts`, `main/locust-mcp-server.ts`, `main/locust-mcp-tools.ts`, `main/cli-session-import.ts`, `main/updater-service.ts`, `packages/cli/src/*.ts` | 2,845 | Area 13 | Native localhost HTTP server hosting `/tools/call` and writing `locust-mcp-connection.json` (so external MCP clients and `--ask` work against `locust-native` directly), external CLI transcript importers, and Sparkle/native auto-updater. |
| **15. Window Chrome, Tray, Pets & Document Viewers (PDF/Office)** | `main/tray-service.ts`, `main/window-chrome.ts`, `main/window-glass.ts`, `main/pet-library.ts`, `main/pdf-service.ts`, `main/office-doc-service.ts`, `main/file-viewer-service.ts` | 2,458 | Areas 14, 15 | `NSStatusItem` menu bar tray, `NSVisualEffectView` vibrancy, workspace file reader, `PDFKit` / `calamine` / `zip`+XML readers for PDF, XLSX, and DOCX extraction. |

*Note: Counts in the table above include `codex-mission.ts` and `mission-store` lines that also power Area 1 thread state; every file in `main/` and `packages/` is accounted for exactly once in the Area totals in Section 3.*

---

## 3. Totals: Feature Counts & Non-Test Line Counts

### 3.1 Feature Parity Summary by Area

| # | Area | Done | Partial | Missing | Total Features | TS UI + Shared Lines (`renderer`+`shared`+`preload`) | TS Engine Lines (`main`+`packages`) | Total TS Non-Test Lines | Native Rust Non-Test Lines |
| :- | :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | Conversations & Thread Rendering | 3 | 5 | 12 | **20** | 39,001 | 21,162 | **60,163** | ~1,550 |
| 2 | Composer & Input | 3 | 1 | 8 | **12** | 4,994 | 689 | **5,683** | ~420 |
| 3 | Teammates, Rooms, Memory & Skills | 2 | 0 | 8 | **10** | 8,288 | 8,796 | **17,084** | ~260 |
| 4 | Runtimes, Routing & Fallback | 2 | 1 | 8 | **11** | 4,696 | 25,874 | **30,570** | ~380 |
| 5 | Approvals & Permissions | 0 | 0 | 8 | **8** | 4,610 | 3,106 | **7,716** | 0 |
| 6 | Changes Panel, Diffs, Git, Worktrees, PRs & Scout | 0 | 0 | 10 | **10** | 6,345 | 6,086 | **12,431** | 0 |
| 7 | Terminal & Own Commands | 0 | 0 | 4 | **4** | 762 | 1,736 | **2,498** | 0 |
| 8 | Built-in Browser & Page Previews | 0 | 0 | 5 | **5** | 1,552 | 2,500 | **4,052** | 0 |
| 9 | Routines, Compare & Benchmarks | 1 | 0 | 5 | **6** | 5,862 | 4,610 | **10,472** | ~160 |
| 10 | Calls, Voice Typing & Quick Ask | 2 | 0 | 4 | **6** | 2,919 | 1,374 | **4,293** | 1,150 |
| 11 | Settings, Usage, Privacy & Backup | 0 | 0 | 7 | **7** | 7,337 | 3,593 | **10,930** | 0 |
| 12 | Cloud, Remote, Phone, Chat Apps & Desk | 0 | 0 | 7 | **7** | 5,849 | 9,728 | **15,577** | 0 |
| 13 | Onboarding, Home, Updates & CLI | 2 | 1 | 5 | **8** | 3,876 | 3,564 | **7,440** | ~310 |
| 14 | Pets, Bots & Look / Window Chrome | 1 | 1 | 3 | **5** | 11,700 | 1,157 | **12,857** | ~250 |
| 15 | PDFs, Diagrams, Office Docs & File Viewer | 0 | 1 | 5 | **6** | 2,646 | 1,301 | **3,947** | ~49 |
| | **TOTAL** | **16 (12.8%)** | **10 (8.0%)** | **99 (79.2%)** | **125** | **110,437** | **95,276** | **205,713** | **4,329** |

---

## 4. Suggested Order of Rounds (Daily-Path First, ≤ ~1,500 Lines of Rust / Day)

Each round is scoped so a single engineer can build, test, and land it in one day (~800–1,500 non-test lines of Rust).
- **Phase A (Rounds 1–5)** completes the daily-driver UI & ledger fidelity while still able to coexist with or transition off Electron.
- **Phase B (Rounds 6–12)** builds the **standalone Rust engine** so Electron does not need to run at all.
- **Phase C (Rounds 13–20)** ports the power-user developer tools (Git/Diffs, Terminal, Browser, Routines/Compare, Settings/Usage).
- **Phase D (Rounds 21–25)** ports secondary surfaces (Voice/Calls, Cloud/Anywhere/Chat bridges, Docs/PDFs, Pets/Themes, Onboarding/Updater).

### Phase A: Daily-Driver Thread, Composer & Ledger Fidelity
1. **Round 1 — Rich Thread Blocks & Ledger Event Completion (~1,200 LOC)**
   - **Scope**: Extend `src/ledger.rs` and `src/main.rs` to parse and render expandable tool input/output drawers, reasoning/thinking traces with elapsed timers, subagent nested trees (`parentItemId`), peer share cards (`mission.peer`), and `<locust-ask>` / `<locust-file>` / `<locust-next>` interactive blocks.
2. **Round 2 — Full Multiline Composer, Attachments, Mentions & Queue (~1,300 LOC)**
   - **Scope**: Upgrade main-window composer to multiline (`Shift+Enter`), persistent per-conversation disk drafts, `@file` / `@teammate` autocomplete popup, `/` skill & slash-command menu, image/file paste & drag-and-drop, and follow-up message queue while a turn is running.
3. **Round 3 — Approvals & Permission Cards End-to-End (~1,100 LOC)**
   - **Scope**: Parse `mission.approval` events in `ledger.rs`, render inline Approval cards in `main.rs` and `quick.rs` (`Allow once`, `Allow session`, `Deny`), add sidebar pending-approval badge, and wire approval responses back to the engine.
4. **Round 4 — Sidebar Organization, Search & Multi-Pane Threads (~1,300 LOC)**
   - **Scope**: Add conversation search (`Cmd+F` in thread + global sidebar search), pinning/archiving/renaming/deleting conversations, workspace/folder switcher, and 2-up split thread panes.
5. **Round 5 — Code Blocks, File Preview Drawer & Command Palette (~1,100 LOC)**
   - **Scope**: Add syntax-highlighted code blocks with copy/path headers, clickable `file://` links opening a right-hand file reader drawer (with line-range highlighting), and `Cmd+K` command palette.

### Phase B: Standalone Rust Engine (Zero-Electron Independence)
6. **Round 6 — Native Ledger Writer, Mission Store & Local MCP Host (~1,400 LOC)**
   - **Scope**: Add atomic JSONL ledger creation/append (`mission.created`, `mission.event`, continuation chains) and host the localhost HTTP MCP server (`locust-mcp-connection.json`) directly inside `locust-native` so it no longer requires Electron's server.
7. **Round 7 — Native Teammate, Memory & Skill Stores (~1,200 LOC)**
   - **Scope**: Read/write `<userData>/teammates.json` (plus starter presets Argon, Codec, Scout, Sage), shared team memory (`<locust-memory>` parser & store), and `SKILL.md` / `CLAUDE.md` / `AGENTS.md` workspace discovery.
8. **Round 8 — Standalone Runtime Driver: Codex & Claude Code (~1,500 LOC)**
   - **Scope**: Build the native async runtime adapter trait and implement subprocess/JSON-RPC streaming drivers for **Codex** (`codex app-server` / CLI) and **Claude Code**, piping events directly into the native ledger writer.
9. **Round 9 — Standalone Runtime Driver: Gemini, OpenCode & Local/API Models (~1,400 LOC)**
   - **Scope**: Add adapters for Gemini CLI, Antigravity, OpenCode, Cursor/Aider, and direct HTTP OpenAI/Anthropic/Ollama endpoints, plus runtime health/auth detection (`runtime-doctor`).
10. **Round 10 — Native Routing, Model Catalog, Multi-Account & Carry-On (~1,300 LOC)**
    - **Scope**: Port `model-catalog`, smart task routing, rate-limit detector, multi-account pool rotation, and cross-runtime handoff packet generation in pure Rust.
11. **Round 11 — Native Command Guard, Permission Host & OS Keychain (~1,200 LOC)**
    - **Scope**: Port shell command safety analysis, workspace path boundary checks, secret leak scanner, saved permission rules store, and macOS Keychain (`security-framework`) secret storage.
12. **Round 12 — Multi-Teammate Rooms & Peer Share Relay (~1,200 LOC)**
    - **Scope**: Port `peer-share-relay` (`<locust-share to="...">` auto-dispatch between teammates), multi-teammate Rooms/Groups turn orchestration, and Teammate create/edit UI modal.

### Phase C: Developer Workflow Tools (Git, Terminal, Browser, Routines, Settings)
13. **Round 13 — Changes Panel, Unified/Split Diff Viewer & Turn Checkpoints (~1,400 LOC)**
    - **Scope**: Build right-hand Changes panel, git status watcher, syntax-highlighted unified/split diff viewer with hunk stage/revert, and pre-turn git checkpoints with one-click Undo.
14. **Round 14 — Git Branch/Commit, Isolated Worktrees & Post-Turn Edit Check (~1,400 LOC)**
    - **Scope**: Add branch switcher, commit/push UI, per-mission isolated git worktree lifecycle + merge-back, and post-turn `mission.edit_check` (auto lint/typecheck/test runner).
15. **Round 15 — Pull Requests, Issues & Scout Diff Review (~1,200 LOC)**
    - **Scope**: Port GitHub/GitLab PR status/create/merge panel, GitHub/Linear issue picker, and Scout automated diff review findings.
16. **Round 16 — Integrated PTY Terminal Drawer & Own Commands (~1,300 LOC)**
    - **Scope**: Embed native PTY (`portable-pty`) + ANSI terminal grid in a collapsible bottom drawer, `!` inline user commands in thread, and background dev-server process list.
17. **Round 17 — Built-in Web Preview Pane & Visual Element Picker (~1,300 LOC)**
    - **Scope**: Embed `WKWebView` preview pane via `rune-surface`, localhost port auto-detect, DOM element picker bridge to composer, and console/network error capture.
18. **Round 18 — Scheduled Routines, File Watchers & Hooks (~1,300 LOC)**
    - **Scope**: Port cron/interval routine scheduler, `notify` file-watcher triggers, git hook triggers, and the Routines management screen.
19. **Round 19 — Side-by-Side Compare Mode, AI Judge & Turn Reruns (~1,300 LOC)**
    - **Scope**: Port parallel worktree Compare execution (2–4 models/teammates), AI Judge rubric scoring card, pick-winner worktree apply, and single-turn Rerun/Tune modal.
20. **Round 20 — Settings Screen, Usage/Cost Analytics & Backup (~1,400 LOC)**
    - **Scope**: Build the categorized Settings screen, token/cost rollup dashboard (`usage-ledger`), external MCP client server config UI, and workspace backup/restore.

### Phase D: Voice, Cloud/Remote, Documents & Polish
21. **Round 21 — Push-to-Talk Voice Dictation & Realtime Voice Calls (~1,400 LOC)**
    - **Scope**: Add `CoreAudio` mic capture, Whisper/STT dictation in composer and Quick Ask, and bidirectional Realtime Voice Call overlay with live transcript logging.
22. **Round 22 — Locust Anywhere (Phone/Web Pairing) & Chat App Bridges (~1,400 LOC)**
    - **Scope**: Port the Locust Anywhere QR pairing/relay server and Telegram, Discord, and Slack bot bridges.
23. **Round 23 — Cloud Computers, Background Runs & Desk View (~1,300 LOC)**
    - **Scope**: Port Cloud computer/task orchestration, Claude Cloud background run syncing, and the spatial Desk overview screen.
24. **Round 24 — PDF, Spreadsheet, DOCX & Mermaid Diagram Viewers (~1,200 LOC)**
    - **Scope**: Add native PDF viewer (`PDFKit`), XLSX/CSV table viewer, DOCX reader, and Mermaid diagram rendering inside threads and the file drawer.
25. **Round 25 — First-Run Onboarding, Home Briefing, Tray, Pets & Auto-Updater (~1,300 LOC)**
    - **Scope**: Build first-run setup wizard, Home screen briefing, macOS menu bar tray icon (`NSStatusItem`), animated sidebar Pets/Bots, custom themes/glass, and native auto-updater.

---

## 5. Verification & Unconfirmed Items

- **Confirmed directly from source code**:
  - All 4 Rust modules in `/Users/locust/work/locust-wt/native/apps/native/src` (`main.rs`, `quick.rs`, `ledger.rs`, `engine.rs`) and `README.md`.
  - All 736 non-test TypeScript/TSX files across `/Users/locust/work/locust/apps/desktop/src/{renderer,main,shared,preload}` and `/Users/locust/work/locust/packages/{runtime-adapters,mission-store,cli}`, plus `/Users/locust/work/locust/CHANGELOG.md` (through `0.871.0`).
- **Unconfirmed / Not Executed (Read-Only Constraints)**:
  - Did not run `cargo test` or launch `locust-native` against a live Electron instance during this audit (read-only code inspection).
  - Exact Rust line counts in Section 4 are engineering estimates based on the current compactness ratio in `locust-native` (~4,329 lines of Rust covering ~15–20% of the core thread/quick-ask/ledger/MCP client surface).
