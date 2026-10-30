// H1: first, before any import can spawn -- no bare name is found in its working folder.
import './no-planted-executables.js'
import { heldChord } from './composer-chords.js'
import { editorsOnPath, launchEditor } from './installed-editors.js'
import { defaultAppMayOpen } from '../shared/file-menu.js'
import { columnInstructions, compareInstructionsRefusal, compareSkillRefusal, shuffledColumns } from './compare-instructions.js'
import { tuneDraftPrompt } from './tune-drafts.js'
import { columnChangesSummary, parseTuneLesson, tuneLessonPrompt, type TuneLessonSetup } from './tune-lessons.js'
import { createTuneLessonStore } from './tune-lesson-store.js'
import { combineSetups, keptSkill, otherSkill, parseTuneCombine, tuneMeasuredOf, tuneSkillOf, TEAMMATE_SKILL_LIST_CHANNEL, TEAMMATE_SKILL_REMOVE_CHANNEL, TEAMMATE_SKILL_USE_CHANNEL, TUNE_COMBINE_CHANNEL, TUNE_COMBINE_FALLBACK, TUNE_DRAFT_CHANNEL, TUNE_LESSON_FALLBACK, TUNE_LESSON_LIST_CHANNEL, TUNE_LESSON_NOTE_CHANNEL, TUNE_LESSON_REMOVE_CHANNEL } from '../shared/tune.js'
import { tuneCombinePrompt } from './tune-combine.js'
import { createClaudeContextReader, prepareClaudeContextLaunch, recordedClaudeContextLaunch, type ClaudeContextReader } from './claude-context.js'
import { CONTEXT_BREAKDOWN_CHANNEL } from '../shared/context-breakdown.js'
import { AGENT_ACCOUNTS_CHANNEL, accountFromRoute, supportsAccounts, type AccountAction } from '../shared/agent-accounts.js'
import { createAgentAccounts } from './agent-accounts.js'
import { startAccountLogin } from './agent-account-login.js'
import { OrganizationPolicyError, readOrganizationPolicy } from './organization-policy.js'
import { createSpendRefusal } from './organization-spend.js'
import { policyCloudTerminalRefusal, policyFeatureRefusal, policyModels, policyTurnRefusal, policyUnmeteredRefusal, type PolicyTurn } from '../shared/organization-policy.js'
import { randomUUID } from 'node:crypto'
import { startAppServerProcess } from './app-server-process.js'
import { ownGitArgs } from './git-guard.js'
import { APP_USER_MODEL_ID, DEVELOPMENT_APP_USER_MODEL_ID, mayShowToasts, repairStartMenuShortcut, sweepStaleElectronShortcuts } from './stale-shortcut.js'
import { openingPlacement, readSavedWindow } from './window-bounds.js'
import type { SavedWindow } from './window-bounds.js'
import { app, BrowserWindow, clipboard, crashReporter, dialog, globalShortcut, ipcMain, Menu, nativeImage, nativeTheme, Notification, powerMonitor, powerSaveBlocker, protocol, safeStorage, screen, session, shell, Tray } from 'electron'
import { createVoiceHost } from './voice-host.js'
import { microphoneRunsOn, voicePermission } from './voice-permission.js'
import { windowChrome } from './window-chrome.js'
import { macMenuTemplate } from './mac-menu.js'
import { VOICE_READY, VOICE_DOWNLOAD, VOICE_TRANSCRIBE, VOICE_CANCEL, VOICE_PROGRESS, VOICE_SETTINGS_READ, VOICE_SETTINGS_SAVE, VOICE_OPENAI_CONSENT } from '../shared/voice.js'
import { TALK_SHORTCUT_PENDING, TALK_SHORTCUT_PRESSED, TALK_SHORTCUT_READ, TALK_SHORTCUT_SET, isTalkShortcut, type TalkShortcutPress } from '../shared/talk-shortcut.js'
import { LOCUST_LINK_TAKE, LOCUST_LINK_WAITING, linkInArguments, parseLocustLink, type LocustLink } from '../shared/locust-link.js'
import { claimLinks, createLinkInbox } from './locust-links.js'
import { createOpenCodeFirstStart, OPENCODE_SETUP_TIMEOUT_MS } from './opencode-first-start.js'
import { createPlainCheckpoints } from './checkpoints-plain.js'
import { createTalkShortcut, type TalkShortcut } from './talk-shortcut.js'
import { createScout, type Scout } from './scout.js'
import { createScoutFixes, type ScoutFixes } from './scout-fix.js'
import { folderMarkOf } from './scout-folder.js'
import { readScoutGithub, type ScoutGithubUnread } from './scout-github.js'
import { createScoutAway, SCOUT_AWAY_FILE, type ScoutAwayRelay } from './scout-away.js'
import { phoneScoutCards } from './phone-scout-cards.js'
import { SCOUT_AWAY_CHANGED, SCOUT_AWAY_SET, SCOUT_AWAY_STATUS } from '../shared/scout-away.js'
import { createCommentReader, postComment } from './scout-comments.js'
import { inQuietHours, SCOUT_CHANGED, SCOUT_POST, SCOUT_EDIT_POST, SCOUT_DISMISS, SCOUT_FIX_AGAIN, SCOUT_FIX_APPLY, SCOUT_FIX_SKIP, SCOUT_LOG, SCOUT_LOOK, SCOUT_NOTES, SCOUT_SET, SCOUT_STATE, SCOUT_TUNE, SCOUT_TURN_ON, type ScoutQuietHours } from '../shared/scout.js'
import { FREE_MODELS_BEST_FIRST } from '../shared/free-models.js'
import { DEFAULT_CHATGPT_VOICE, VOICE_CALL_ANSWER, VOICE_CALL_AVAILABLE, VOICE_CALL_EVENT, VOICE_CALL_PREVIEW, VOICE_CALL_SET_VOICE, VOICE_CALL_START, VOICE_CALL_STOP, VOICE_CALL_TOOLS, VOICE_CALL_VOICES, VOICE_PREVIEW_SAVE, VOICE_PREVIEW_SAVED, VOICE_PREVIEW_TRIGGER, isChatGptVoice, isVoiceCallOffer, isVoiceCallTarget, talkRunsOn, type ChatGptVoice, type VoiceAddOn, type VoiceCallTarget } from '../shared/voice-call.js'
import { createVoiceCalls } from './voice-call.js'
import { createCallLog } from './call-log.js'
import { CALLS_LIST, CALLS_SAVE } from '../shared/calls.js'
import { CALL_FOLDER_TOOLS, callFolderTools } from './voice-call-folder.js'
import { createVoiceSettingsStore } from './voice-settings.js'
import { MCP_CONNECTION_FILE, createLocustMcpHost } from './locust-mcp-host.js'
import { claudeAppConfigFile, claudeAppStartedAt, createClaudeAppConnector, relaunchClaudeApp } from './claude-app-connection.js'
import { createLocustMcpTools } from './locust-mcp-tools.js'
import { createLocustCliSessions } from './locust-cli-sessions.js'
import { REMOTE_MCP_CHANGED, REMOTE_MCP_DECIDE, REMOTE_MCP_GET, REMOTE_MCP_REMOVE, REMOTE_MCP_SET } from '../shared/remote-mcp.js'
import { createRemoteMcp, REMOTE_MCP_FILE } from './remote-mcp.js'
import { LOCUST_MCP_CLAUDE_APP, LOCUST_MCP_CLAUDE_APP_RESTART, LOCUST_MCP_CLAUDE_APP_SET, LOCUST_MCP_GET, LOCUST_MCP_SET, LOCUST_MCP_SET_CONVERSATIONS, LOCUST_MCP_SET_EDITOR_ANSWERS, LOCUST_MCP_SET_OWN_MODE } from '../shared/locust-mcp.js'
import { createPageServer, fromPagePreview, pageMayReach, PAGE_SCHEME } from './page-preview.js'
import { CANCEL_SCRIPT, captureRectOf, pageFrameOf, pickInFrame } from './page-pick.js'
import { createRuntimeCommands } from './runtime-commands.js'
import { createCursorDefaultModel } from './cursor-default-model.js'
import { listWorkspaceFiles } from './workspace-files.js'
import { MAX_TAGGED, taggedPrompt } from '../shared/tagging.js'
import { COMPARE_SLOTS, COMPARE_TREES_DIRECTORY, comparesGoneWith, compareNeedsCopy, compareRefusalOf, compareSlotKey, compareTreeId, MAX_COMPARE_SLOTS, MAX_JUDGE_CRITERIA, MIN_COMPARE_SLOTS, PANEL_JUDGES } from '../shared/compare.js'
import { criteriaLines, draftedPoints, pointsDraftRequest } from '../shared/compare-score.js'
import { reverseChanges } from '../shared/reverse-diff.js'
import { createCloudTaskService, launchRunner } from './cloud-task-service.js'
import { environmentOf, githubRepoOf, leavingNoLog } from './cloud-tasks.js'
import { createCloudComputers } from './cloud-computer.js'
import { createCloudTurns } from './cloud-turns.js'
import { isCloudAgent, type CloudAgent } from '../shared/cloud-computer.js'
import { choiceArgs, createClaudeCloud } from './claude-cloud.js'
import { createRemoteControl } from './claude-remote-control.js'
import { registerRemoteControlIpc } from './remote-control-ipc.js'
import { runInPseudoTerminal } from './pseudo-terminal.js'
import type { Runner } from './cloud-tasks.js'
import type { ReverseChange } from '../shared/reverse-diff.js'
import { bringInCopy, compareRoot, copyDiff, copyLineChanges, copyRefusal, makeCompareCopy, removeCompareCopies } from './compare-copies.js'
import { comparePagePath } from './compare-page-path.js'
import { blindName, hasColumnInstructions, setupName, type CompareRoute, type CompareSlotId, type PublicCompare, type PublicCompareSlot } from '../shared/compare.js'
import { createCompareStore } from './compare-store.js'
import { createCompareReruns } from './compare-reruns.js'
import { createApprovalRuleStore } from './approval-rule-store.js'
import { createFileArrivals, POLL_MS } from './routine-file-watch.js'
import { createRoutineHooks, ROUTINE_HOOKS_FILE, type HookRelay } from './routine-hooks.js'
import { ROUTINE_HOOK_READ, ROUTINE_HOOK_NEW } from '../shared/routine-hook.js'
import { enforcedAnswer, ruleCandidateOf, ruledActionOf, ruleRefusalFor, ruleSentence } from '../shared/approval-rules.js'
import { exactLine } from '../shared/approval-data.js'
import { plainCommand } from '../shared/command-plain.js'
import { scrubSecrets } from '../shared/secrets.js'
import { createRunAlways, decide } from '../shared/who-decides.js'
import { judgeAnswerText, judgePrompt } from './compare-judge.js'
import { keepOutOfGit } from './attachments-for-run.js'
import { REPLY_PAGE_ROOT, writeReplyPage } from './reply-page.js'
import type { JudgedAnswer, JudgedCheck } from './compare-judge.js'
import electronUpdater from 'electron-updater'

const { autoUpdater } = electronUpdater
import {
  createNodeProbeRunner,
  createClaudeCommandListCommand,
  createOpenCodeServeCommand,
  readClaudeCommands,
  readOpenCodeCommands,
  createNodeRuntimeProcessRunner,
  createPathExecutableLocator,
  cursorCanEnforceReadOnly,
  discoverInstalledRuntimes,
  discoverInstalledRuntimesEach,
  openCodeUsesAcp,
  killSpawnedTree,
  releaseProcessTree
} from '@teammate/runtime-adapters'
import { boundedApproval, createFileMissionLedger, createFileWorkroom } from '@teammate/mission-store'
import { retireSetAsideMessages } from './set-aside-messages.js'
import { MAC_RELEASES_API, newerMacRelease } from './mac-release.js'
import { createMacUpdater, macSelfUpdateTarget } from './mac-self-update.js'
import type { PublicRecoveredMission, AppChangelog, AppChangelogEntry, CodexMissionStartResponse, TeammateRoute, TurnUndoState, WorkspaceSettings } from '../shared/ipc.js'
import type { MissionApproval, MissionLedger, MissionStarter, Workroom } from '@teammate/mission-store'
import type { ClaudeListedModel, RuntimeDiscovery } from '@teammate/runtime-adapters'
import { spawn } from 'node:child_process'
import { appendFileSync, mkdirSync, readFileSync, writeFileSync, existsSync, renameSync, statSync, unlinkSync } from 'node:fs'
import { appendFile, copyFile, lstat, mkdir, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { randomInt } from 'node:crypto'
import { basename, dirname, isAbsolute, join, resolve as resolvePath } from 'node:path'
import { pathToFileURL } from 'node:url'
import { shellPath } from './shell-path.js'
import { linuxInstallOf, linuxPackageUrl, linuxUpdateTestOf } from './linux-install.js'
import { DOWNLOADS, SITE } from '../shared/locust-home.js'
import { readTheme, resolvedTheme, THEME_CHOICES, windowGround, writeTheme } from './appearance.js'
import { APPEARANCE_CHANGED_CHANNEL, APPEARANCE_GET_CHANNEL, APPEARANCE_SET_CHANNEL, THEME_ARGUMENT } from '../shared/appearance.js'
import type { AppearanceState, ThemeChoice } from '../shared/appearance.js'
import { homedir, hostname, release } from 'node:os'
import { execFile } from 'node:child_process'
import { createVoicePreviews, speakHello } from './voice-preview.js'
import { setTimeout as delay } from 'node:timers/promises'
import { MAX_PROMPT_LENGTH, createCodexMissionService, resumableThreadOf, runtimeThreadIdOf } from './codex-mission.js'
import { chosenLeaveOut, howTurnEnded } from './handoff.js'
import { officeWordsSection } from './office-words.js'
import { createRoutineCopies } from './routine-copy.js'
import { openInTerminal, openProgramInTerminal, terminalRequestFor } from './open-in-terminal.js'
import { createTerminalHost, terminalShells } from './terminal-host.js'
import { registerTerminalIpc } from './terminal-ipc.js'
import { createOwnCommandHost } from './own-command-host.js'
import { registerOwnCommandIpc } from './own-command-ipc.js'
import { terminalFolder } from './terminal-folder.js'
import { TERMINAL_EVENT } from '../shared/terminal.js'
import { policyBrowserRefusal, policyTerminalRefusal } from '../shared/organization-policy.js'
import { createBrowserHost } from './browser-host.js'
import { registerBrowserIpc } from './browser-ipc.js'
import { BROWSER_EVENT_CHANNEL, BROWSER_POINT_HOTKEY_CHANNEL } from '../shared/browser.js'
import { catchUpTerminal, createTerminalCatchUp, createTerminalImports, createTranscriptReader, transcriptPathFor } from './terminal-catch-up.js'
import type { CatchUpFacts } from './terminal-catch-up.js'
import { importSession, listImportableSessions } from './session-import.js'
import { createOpenCodeSessionReader, cursorExchanges, listCursorSessions } from './agent-session-import.js'
import type { ImportableSession } from './session-import.js'
import { readTextChunk, withTextChunk } from './png-text.js'
import { recordTurns, missionRecordMarkdown, rawRecordJson, recordFileName } from './mission-export.js'
import { TEAM_CARD_KEYWORD, freeName, readTeamCard, readTeamCardRoutines, teamCardOf } from '../shared/team-card.js'
import { cardRoutineFile, teamCardRoutines } from './team-card-routines.js'
import { approvalAnswerFrom, createApprovalChannel } from './approval-channel.js'
import { PeerRecordError } from './peer-exchange.js'
import { readNpmBinDirectory } from './npm-prefix.js'
import { bundledNpmBinDirectory, bundledNpmPrefix, findBundledNpm } from './bundled-npm.js'
import { createModelCatalog } from './model-catalog.js'
import { describeGone, diagnosticLine, shouldRoll, startupDetail } from './diagnostics.js'
import { createGroupStore } from './group-store.js'
import { createQueuedMessageStore } from './queued-message-store.js'
import { QUEUED_MESSAGES_READ_CHANNEL, QUEUED_MESSAGES_WRITE_CHANNEL } from '../shared/queued-messages.js'
import { createBriefSessions } from './brief-sessions.js'
import { createRunEnd } from './run-end.js'
import { createCarryOn, type CarryOn } from './carry-on.js'
import { createCheckFix, type CheckFix } from './check-fix.js'
import { parsedCheckFixTries } from '../shared/check-fix.js'
import { CARRY_ON_CHANNEL, parsedCarryOnRoute } from '../shared/carry-on.js'
import { createPhoneAlerts, type PhoneAlert } from './phone-alerts.js'
import { createPhoneAnywhere, DEFAULT_RELAY, githubUserOf } from './phone-anywhere.js'
import { createPhoneHost, type PhoneCalls } from './phone-host.js'
import { createTelegramLink, TELEGRAM_API } from './telegram.js'
import { TELEGRAM_CHANGED_CHANNEL, TELEGRAM_NEW_LINK_CHANNEL, TELEGRAM_REMOVE_CHANNEL, TELEGRAM_SET_TOKEN_CHANNEL, TELEGRAM_SETTINGS_CHANNEL } from '../shared/telegram.js'
import type { ChatLink, ChatLinkOptions, ChatStart } from './chat-link.js'
import { createDiscordLink, DISCORD_API } from './discord.js'
import { createSlackLink, SLACK_API } from './slack.js'
import { CHAT_ALERTS_CHANNEL, type ChatAppSettings } from '../shared/chat-apps.js'
import { DISCORD_CHANGED_CHANNEL, DISCORD_NEW_LINK_CHANNEL, DISCORD_REMOVE_CHANNEL, DISCORD_SET_TOKEN_CHANNEL, DISCORD_SETTINGS_CHANNEL } from '../shared/discord.js'
import { SLACK_CHANGED_CHANNEL, SLACK_NEW_LINK_CHANNEL, SLACK_REMOVE_CHANNEL, SLACK_SET_TOKEN_CHANNEL, SLACK_SETTINGS_CHANNEL } from '../shared/slack.js'
import { countingLedgerWrites, keyedOnce } from './ledger-writes.js'
import { acpScratch, createAcpAgents } from './acp-agents.js'
import { acpCatalog, ACP_AGENTS_LIST_CHANNEL, ACP_AGENTS_REMOVE_CHANNEL, ACP_AGENTS_SAVE_CHANNEL, ACP_AGENTS_TEST_CHANNEL } from '../shared/acp-agents.js'
import { phonePage } from './phone-page.js'
import { PHONE_GITHUB_IN_CHANNEL, PHONE_GITHUB_OUT_CHANNEL, PHONE_OPEN_BROWSER_CHANNEL } from '../shared/phone.js'
import { PHONE_ANYWHERE_CHANGED_CHANNEL, PHONE_ANYWHERE_CHANNEL, PHONE_FORGET_CHANNEL, PHONE_MODES, PHONE_NEW_CODE_CHANNEL, PHONE_NIGHT_CHANNEL, PHONE_REMOVE_CHANNEL, PHONE_SET_ENABLED_CHANNEL, PHONE_SET_MODE_CHANNEL, PHONE_SETTINGS_CHANNEL, phoneConversations, phoneTurns, type PhoneAnywhereSettings, type PhoneMode, type PhoneModeState } from '../shared/phone.js'
import { createKeepAwake, KEEP_AWAKE_BEAT_MS } from './keep-awake.js'
import { attentionDot, needsYouCountFrom, taskbarAttention } from './taskbar-attention.js'
import { CLOSE_BUTTONS, closeQuestion, hideToTrayOnClose, shouldAskBeforeClosing, trayLine, trayMenuLabels } from './quit-guard.js'
import { BACKGROUND_ARG, createLoginItem } from './login-item.js'
import { nextRoutineDueLine } from '../shared/routine-schedule.js'
import { AWAKE_FILE, BACKGROUND_FILE, readKeepRunning, readStayAwake, writeKeepRunning, writeStayAwake } from './keep-running.js'
import { AWAY_FILE, readAttentionMark, writeAttentionMark } from './away.js'
import { awayCountsFrom, awayLine, wasAway } from '../shared/away.js'
import type { AwaySummaryCounts } from '../shared/away.js'
import { createRecentEdits } from './recent-edits.js'
import { createCheckpoints, createTurnRecords, type TurnRecords } from './checkpoints.js'
import { clearClaudeSkillCopies, prepareClaudeSkills } from './claude-skills.js'
import { prepareRunSkills } from './run-skills.js'
import { takesSkills } from '../shared/skill-runtimes.js'
import { createSkillLibrary, SkillLibraryError } from './skill-library.js'
import { createFolderCommits } from './folder-commit.js'
import { createBackgroundRuns } from './claude-background-runs.js'
import type { BackgroundRun, BackgroundRuns } from './claude-background-runs.js'
import { BACKGROUND_DISMISS_CHANNEL, BACKGROUND_LIST_CHANNEL, BACKGROUND_OPEN_CHANNEL, BACKGROUND_SETUP_CHANNEL, BACKGROUND_START_CHANNEL, BACKGROUND_STOP_CHANNEL } from '../shared/background.js'
import type { BackgroundStartResponse, PublicBackgroundRun } from '../shared/background.js'
import { FOLDER_CHANGES_CHANNEL, FOLDER_COMMIT_CHANNEL, FOLDER_REPOSITORY_CHANNEL, type CommitResult, type FolderChanges } from '../shared/folder-commit.js'
import { GITHUB_ISSUE_OPEN_CHANNEL, GITHUB_ISSUE_REMEMBERED_CHANNEL, GITHUB_ISSUES_LIST_CHANNEL, GITHUB_ISSUES_READY_CHANNEL, startedIssue } from '../shared/github-issue.js'
import { checkoutIssueBranch, createGithubIssues, gitText } from './github-issues.js'
import { readRuntimeArtifacts } from './runtime-artifacts.js'
import { relative } from 'node:path'
import { decideReveal, decideTeammateFile, insideOnDisk, readRoots } from './reveal-file.js'
import { readWorkspaceImage } from './workspace-image.js'
import { readPdfPages } from './pdf-pages.js'
import { addConnector, removeConnector } from './connector-add.js'
import { createGithubAccount, ghCommand } from './github-account.js'
import { FOLDER_PULL_REQUEST_CHANNEL, GITHUB_ISSUE_LINK_CHANNEL, GITHUB_PULL_LINK_CHANNEL, PULL_REQUEST_NEWS_CHANNEL, PULL_REQUEST_WATCH_CHANNEL } from '../shared/pull-request.js'
import { createPullRequestWatch } from './pull-request-watch.js'
import { createPullLinkReader } from './pull-link.js'
import { FOLDER_DIFF_CHANNEL } from '../shared/folder-diff.js'
import { GITHUB_ACCOUNT_CHANNEL, GITHUB_CLI_VERSIONS_CHANNEL, GITHUB_INSTALL_CHANNEL, GITHUB_UPDATE_CHANNEL, GITHUB_SIGN_IN_CANCEL_CHANNEL, GITHUB_SIGN_IN_CHANNEL, GITHUB_SIGN_IN_CODE_CHANNEL } from '../shared/github-account.js'
import type { AgentLaunch } from './connector-add.js'
import { CONNECTOR_ADD_CHANNEL, CONNECTOR_REMOVE_CHANNEL, isConnectorAgent } from '../shared/connector-add.js'
import type { ConnectorAgent } from '../shared/connector-add.js'
import { MAX_ATTACHMENTS } from '../shared/attachments.js'
import { ATTACHMENT_DIR, attachmentDestination, excludeWith } from './attach-outside.js'
import { extensionOf, isViewableText, MAX_TEXT_BYTES, viewerMode } from '../shared/text-files.js'
import { aboutYouSection, MAX_ABOUT_YOU_SUGGESTIONS } from '../shared/about-you.js'
import { SHEET_EXTENSIONS, csvWorkbook } from '../shared/sheet.js'
import { MAX_OFFICE_FILE_BYTES, OFFICE_EXTENSIONS } from '../shared/office-document.js'
import { readDocx, readPptx } from './office-text.js'
import { WorkbookUnreadable, readXlsx } from './xlsx.js'

/** A spreadsheet the viewer reads may be this big on disk; the grid it draws is bounded anyway (0.364). */
const MAX_SHEET_FILE_BYTES = 8 * 1024 * 1024
import { comparableLines, createEditCheck, runCheckCommand } from './edit-check.js'
import type { EditCheckResult } from './edit-check.js'
import { createTeammateStore, parsedCheckCommand, TeammateNameTakenError, isTeammateRoute } from './teammate-store.js'
import { createRoutineStore } from './routine-store.js'
import { createRoutineIO } from './routine-io.js'
import { createRoomStore, exchangeOfRoomPost, recipientsOf } from './room-store.js'
import { createRoomTasks } from './room-tasks.js'
import type { RoomTasks } from './room-tasks.js'
import { boardLines, fittedTaskSection, rowToClaimAtStart } from '../shared/room-task.js'
import { ROOM_HISTORY_POSTS } from '../shared/room-history.js'
import type { RoomHistory, RoomHistoryAnswer } from '../shared/room-history.js'
import { createRoutineRunner } from './routine-runner.js'
import { projectNamedIn } from './named-folder.js'
import { createMemoryStore } from './memory-store.js'
import { WorktreeHasChangesError, createWorktreeManager, defaultRunGit } from './worktrees.js'
import { copilotMcpServerNames, readRuntimeSetup } from './runtime-setup.js'
import { folderSentences, readWorkspaceBrief, groupSection, teammateSection } from './workspace-brief.js'
import { createTeammateKeys } from './teammate-keys.js'
import { KEYS_REACH, keysSection, TEAMMATE_KEYS_LIST_CHANNEL, TEAMMATE_KEYS_REMOVE_CHANNEL, TEAMMATE_KEYS_SET_CHANNEL } from '../shared/teammate-keys.js'
import { createConversationChain, groupBriefFor } from './conversation-chain.js'
import { createMemoryReader } from './memory-reader.js'
import { createAttentionReader } from './attention-reader.js'
import { parseDecision } from '../shared/decision.js'
import { createTranscriptTracker } from './peer-exchange.js'
import type { AttentionReader } from './attention-reader.js'
import type { MemoryReader } from './memory-reader.js'
import type { MemoryBriefing } from './peer-exchange.js'
import { briefedMemories, byLastWritten, daysUnused, lastWritten, memorySection } from '../shared/memory.js'
import { createMemoryRecall } from './memory-recall.js'
import { MEMORY_FILE, retireMemoryFile, writeMemoryFile } from './memory-file.js'
import { changedSince } from './memory-provenance.js'

/** Scheduled routines are checked once a minute; the first check waits for runtime discovery. */
const ROUTINE_TICK_MS = 60_000
const ROUTINE_FIRST_TICK_MS = 15_000
import type { RoutineRunner } from './routine-runner.js'
import { readOneMission, deleteMissionRecord, knownDigests, namedByATeammate, newestTurnOf, readMissionHistory, spendByTeammate, usageTurns, withLiveMissionIds } from './mission-history.js'
import { createReferenceStore } from './conversation-references.js'
import { REFERENCES_CHANNEL, type ReferenceRequest } from '../shared/references.js'
import { isUsageRange, usageSummary } from '../shared/usage.js'
import { monthOf } from '../shared/spend.js'
import { createOwnModelStore, OwnModelRefusal, ownModelAddress, ownModelId, testOwnEndpoint } from './own-models.js'
import { createPriceList, openCodePriceListPath } from './listed-prices.js'
import { createTeammateSkillStore } from './teammate-skills.js'
import { gatherTeammateSetup } from './teammate-setup.js'
import { TEAMMATE_SETUP_CHANNEL } from '../shared/teammate-setup.js'
import { changelogPaths, entries as changelogEntries, readChangelog, splashEntries } from './changelog.js'
import type { ChangelogEntry } from './changelog.js'
import type { CodexMissionService } from './codex-mission.js'
import type { MissionPeerContext } from './workroom-briefing.js'
import { createFolderRegistry, recoverFolderPath, sameFolderAtStart } from './folders.js'
import type { FolderContext } from './folders.js'
import { createConnectorReader } from './connector-reader.js'
import { createRelay } from './relay.js'
import { createAttention, finishFrom } from './attention.js'
import { boundedShutdown } from './bounded-shutdown.js'
import { createPermissionHost } from './permission-host.js'
import { codexCommandGuardConfig, otherCodexHooks, writeCommandGuard, writeCopilotCommandGuard, writeOpenCodeCommandGuard, writeOpenCode2CommandGuard } from './command-guard.js'
import { writeOpenCodeGitReads } from './opencode-git-reads.js'
import { isInsideDirectory, notATeammateFolder, readConfirmedFolders, readRememberedWorkspace, resolveWorkspacePath, sameFolder, WORKSPACE_ARGUMENT, writeConfirmedFolders, writeRememberedWorkspace, workspaceIdFor } from './workspace.js'
import { createUnwrittenAnswers } from './approval-record-note.js'
import { createRaisedApprovals } from './raised-approvals.js'
import { createOutstandingChecks } from './outstanding-checks.js'
import { FIRST_SCREEN_DEADLINE_MS, keepWhatWasAnswered, settleWithin, type SweepCheck } from './sweep-settle.js'
import { createAntigravityHostProbe } from './antigravity-host.js'
import { AntigravityStartError, antigravityStartRefusal, createAntigravityMissionService } from './antigravity-mission.js'
import type { Relay } from './relay.js'
import { createRuntimeDiscoveryService, RUNTIME_DISCOVERY_CHANNEL } from './runtime-discovery.js'
import { bootOutcome, createDiscoveryLog } from './discovery-log.js'
import { createRuntimeFactsStore } from './runtime-facts.js'
import { createAcpCapabilitiesStore } from './acp-capabilities.js'
import { createRuntimeInstaller } from './runtime-installer.js'
import { openSignIn } from './runtime-sign-in.js'
import { FREE_ONLY_REFUSAL, freeRoutesOnly, isFreeRoute } from './free-routes.js'
import {
  FIRST_LOOK_AFTER_MS,
  createRuntimeUpdates,
  mayUpdateAgents,
  npmGlobalRoot,
  npmRelease,
  canaryHeldVersions,
  processesUsing,
  savedUpdatesFrom
} from './runtime-updates.js'
import { createStartReadiness } from './start-readiness.js'
import { plannedRuntimes } from '../shared/runtime-integration.js'
import {
  CODEX_MISSION_CANCEL_CHANNEL,
  CODEX_MISSION_START_CHANNEL,
  CODEX_MISSION_UPDATE_CHANNEL,
  MISSION_APPROVAL_CHANNEL,
  MISSION_APPROVAL_DECIDE_CHANNEL,
  MISSION_APPROVAL_WITHDRAWN_CHANNEL,
  MISSION_HANDOFF_CHANNEL,
  MISSION_RESUME_CHANNEL,
  APP_INFO_CHANNEL,
  NEEDS_YOU_COUNT_CHANNEL,
  AWAY_SINCE_GET_CHANNEL,
  AWAY_SINCE_CHANNEL,
  AWAY_SUMMARY_CHANNEL,
  AWAY_SEEN_CHANNEL,
  RUN_FINISHED_CHANNEL,
  ATTENTION_OPEN_MISSION_CHANNEL,
  APP_CHANGELOG_CHANNEL,
  APP_CHANGELOG_SEEN_CHANNEL,
  APP_UPDATE_CHECK_CHANNEL,
  APP_UPDATE_INSTALL_CHANNEL,
  APP_UPDATE_LANE_CHANNEL,
  KEEP_RUNNING_GET_CHANNEL,
  KEEP_RUNNING_SET_CHANNEL,
  STAY_AWAKE_GET_CHANNEL,
  STAY_AWAKE_SET_CHANNEL,
  LOGIN_ITEM_GET_CHANNEL,
  LOGIN_ITEM_SET_CHANNEL,
  APP_UPDATE_STATE_CHANNEL,
  MISSION_DELETE_CHANNEL,
  MISSION_PRUNE_CHANNEL,
  PROFILE_BACKUP_CHANNEL,
  PROFILE_LAST_RESTORE_CHANNEL,
  PROFILE_PICK_FOLDER_CHANNEL,
  PROFILE_RESTORE_CHANNEL,
  PROFILE_RESTORE_PREVIEW_CHANNEL,
  MISSION_TRASH_LIST_CHANNEL,
  MISSION_RESTORE_CHANNEL,
  MISSION_TRASH_EMPTY_CHANNEL,
  MISSION_STORAGE_CHANNEL,
  MISSION_HISTORY_CHANNEL,
  MISSION_READ_CHANNEL,
  MODEL_CATALOG_CHANNEL,
  RUNTIME_ARTIFACTS_CHANNEL,
  RUNTIME_INSTALL_CHANNEL,
  RUNTIME_UPDATES_CHANNEL,
  RUNTIME_UPDATES_EVENT_CHANNEL,
  RUNTIME_UPDATES_NOW_CHANNEL,
  TURN_UNDO_STATE_CHANNEL,
  TURN_UNDO_CHANNEL,
  RUNTIME_UPDATES_SET_CHANNEL,
  RUNTIME_INSTALL_PROGRESS_CHANNEL,
  RUNTIME_SIGN_IN_CHANNEL,
  RUNTIME_SIGN_IN_CLOSED_CHANNEL,
  OPEN_IN_TERMINAL_CHANNEL,
  TERMINAL_CATCH_UP_CHANNEL,
  SESSION_IMPORT_LIST_CHANNEL,
  SESSION_IMPORT_CHANNEL,
  TEAM_CARD_SAVE_CHANNEL,
  TEAM_CARD_ADD_CHANNEL,
  MISSION_RECORD_SAVE_CHANNEL,
  TEAMMATE_ASSIGN_CHANNEL,
  TEAMMATE_RENAME_MISSION_CHANNEL,
  TEAMMATE_PIN_MISSION_CHANNEL,
  TEAMMATE_SETTLE_MISSION_CHANNEL,
  GROUP_LIST_CHANNEL,
  GROUP_CREATE_CHANNEL,
  GROUP_RENAME_CHANNEL,
  GROUP_REMOVE_CHANNEL,
  GROUP_ASSIGN_CHANNEL,
  GROUP_INSTRUCTIONS_CHANNEL,
  GROUP_ROUTE_CHANNEL,
  ROUTINE_LIST_CHANNEL,
  ROUTINE_CREATE_CHANNEL,
  ROUTINE_UPDATE_CHANNEL,
  ROUTINE_REMOVE_CHANNEL,
  ROUTINE_PAUSE_CHANNEL,
  ROUTINE_RUN_CHANNEL,
  ROUTINE_FOLDER_CHANNEL,
  ROUTINE_EXPORT_CHANNEL,
  ROUTINE_IMPORT_PREVIEW_CHANNEL,
  ROUTINE_TEMPLATE_PREVIEW_CHANNEL,
  ROUTINE_TEMPLATES_CHANNEL,
  ROUTINE_IMPORT_CHANNEL,
  ROUTINE_SETTLE_CHANNEL,
  MEMORY_LIST_CHANNEL,
  MEMORY_ADD_CHANNEL,
  MEMORY_UPDATE_CHANNEL,
  MEMORY_REMOVE_CHANNEL,
  MEMORY_CLEAR_CHANNEL,
  MEMORY_RESTORE_CHANNEL,
  SKILL_LIBRARY_PREVIEW_CHANNEL,
  SKILL_LIBRARY_INSTALL_CHANNEL,
  SKILL_LIBRARY_LIST_CHANNEL,
  SKILL_LIBRARY_REMOVE_CHANNEL,
  RUNTIME_SETUP_CHANNEL,
  WORKTREE_LIST_CHANNEL,
  WORKTREE_REMOVE_CHANNEL,
  WORKTREE_REVIEW_CHANNEL,
  WORKTREE_TURN_DIFF_CHANNEL,
  WORKTREE_LAND_PREVIEW_CHANNEL,
  WORKTREE_LAND_CHANNEL,
  WORKTREE_RESOLVE_CHANNEL,
  WORKSPACE_SETTINGS_READ_CHANNEL,
  WORKSPACE_SETTINGS_WRITE_CHANNEL,
  CONNECTOR_LIST_CHANNEL,
  TEAMMATE_CONNECTORS_CHANNEL,
  TEAMMATE_OWN_BROWSER_CHANNEL,
  TEAMMATE_FOLDER_CHANNEL,
  WORKSPACE_CHOOSE_CHANNEL,
  WORKSPACE_CONFIRM_CHANNEL,
  FOLDER_LIST_CHANNEL,
  SIDE_ASK_CHANNEL,
  FOLDER_SWITCH_CHANNEL,
  CLOUD_WHERE_CHANNEL,
  CLOUD_START_CHANNEL,
  CLOUD_LIST_CHANNEL,
  CLOUD_REFRESH_CHANNEL,
  CLOUD_DIFF_CHANNEL,
  CLOUD_APPLY_CHANNEL,
  CLOUD_FOLDERS_CHANNEL,
  CLOUD_TURNS_CHANNEL,
  CLOUD_TURN_PATCH_CHANNEL,
  CLOUD_TURN_APPLY_CHANNEL,
  CLOUD_TURN_DISCARD_CHANNEL,
  CLOUD_TURN_PULL_REQUEST_CHANNEL,
  CLOUD_COMPUTERS_CHANNEL,
  CLOUD_COMPUTER_STOP_CHANNEL,
  CLOUD_COMPUTER_REMOVE_CHANNEL,
  CLOUD_COMPUTER_TERMINAL_CHANNEL,
  CLOUD_COMPUTER_ALLOW_CHANNEL,
  CLOUD_COMPUTER_FOLDER_CHANNEL,
  CLOUD_COMPUTER_SETUP_CHANNEL,
  CLOUD_COMPUTER_SIGN_IN_CHANNEL,
  CLOUD_COMPUTER_SIGN_IN_STOP_CHANNEL,
  CLOUD_COMPUTER_SIGN_OUT_CHANNEL,
  CLAUDE_CLOUD_START_CHANNEL,
  CLAUDE_CLOUD_LIST_CHANNEL,
  CLAUDE_CLOUD_HOME_CHANNEL,
  CLAUDE_CLOUD_FORGET_CHANNEL,
  CLAUDE_CLOUD_SEND_CHANNEL,
  CLAUDE_CLOUD_CHECK_CHANNEL,
  CLAUDE_CLOUD_APPLY_CHANNEL,
  CLAUDE_CLOUD_TERMINAL_CHANNEL,
  CLAUDE_CLOUD_ENVIRONMENT_CHANNEL,
  REWIND_PUT_BACK_CHANNEL,
  WORKSPACE_ATTACH_CHANNEL,
  WORKSPACE_FILES_CHANNEL,
  USAGE_READ_CHANNEL,
  PDF_PAGES_CHANNEL,
  WORKSPACE_PASTE_CHANNEL,
  WORKSPACE_IMAGE_CHANNEL,
  WORKSPACE_REVEAL_CHANNEL,
  FILE_EDITORS_CHANNEL,
  FILE_OPEN_CHANNEL,
  CLIPBOARD_WRITE_CHANNEL,
  WORKSPACE_SAVE_COPY_CHANNEL,
  WORKSPACE_TEXT_CHANNEL,
  WORKSPACE_PAGE_CHANNEL,
  REPLY_PAGE_CHANNEL,
  PAGE_PICK_CHANNEL,
  PAGE_PICK_CANCEL_CHANNEL,
  RUNTIME_COMMANDS_CHANNEL,
  DIAGNOSTICS_REVEAL_CHANNEL,
  WORKSPACE_OPEN_CHANNEL,
  FEEDBACK_CHANNEL,
  FEEDBACK_EMAIL_CHANNEL,
  FEEDBACK_SAVE_CHANNEL,
  DIAGNOSTICS_REPORT_CHANNEL,
  OPEN_LINK_CHANNEL,
  MAC_RELEASE_CHANNEL,
  APPROVAL_RULES_LIST_CHANNEL,
  APPROVAL_RULES_REMOVE_ALL_CHANNEL,
  APPROVAL_RULES_REMOVE_CHANNEL,
  APPROVAL_RULE_FROM_CARD_CHANNEL,
  HANDOFF_PREVIEW_CHANNEL,
  DEFAULT_RELAY_HOP_CAP,
  DEFAULT_MEMORY_MODE,
  ROOM_LIST_CHANNEL,
  ROOM_CREATE_CHANNEL,
  ROOM_REMOVE_CHANNEL,
  ROOM_RENAME_CHANNEL,
  ROOM_POST_CHANNEL,
  TEAMMATES_TAG_CHANNEL,
  COMPARE_START_CHANNEL,
  COMPARE_ASK_CHANNEL,
  COMPARE_KEEP_CHANNEL,
  COMPARE_RETRY_CHANNEL,
  COMPARE_JUDGE_CHANNEL,
  COMPARE_RERUN_CHANNEL,
  COMPARE_RERUN_CHECKED_CHANNEL,
  COMPARE_DRAFT_POINTS_CHANNEL,
  COMPARE_RERUN_STOP_CHANNEL,
  COMPARE_RERUNS_CHANNEL,
  COMPARE_ADD_MODEL_CHANNEL,
  COMPARE_CHANGES_CHANNEL,
  COMPARE_CHANGES_REFUSAL_CHANNEL,
  COMPARE_IN_PLACE_CHANNEL,
  COMPARE_LIST_CHANNEL,
  RUNTIME_DISCOVERY_EVENT_CHANNEL,
  RUNTIME_DISCOVERY_LOG_CHANNEL,
  SPLASH_DONE_CHANNEL,
  ROOM_TASK_CHANNEL,
  TEAMMATE_CREATE_CHANNEL,
  TEAMMATE_LIST_CHANNEL,
  TEAMMATE_REMOVE_CHANNEL,
  TEAMMATE_UPDATE_CHANNEL,
  TEAMMATE_SPEND_CHANNEL,
  OWN_MODEL_LIST_CHANNEL,
  OWN_MODEL_ADD_CHANNEL,
  OWN_MODEL_REMOVE_CHANNEL,
  OWN_MODEL_TEST_CHANNEL,
  OWN_MODEL_CHAT_ONLY_CHANNEL,
  PET_LIST_CHANNEL,
  PET_SHEET_CHANNEL,
  PET_THUMBNAIL_CHANNEL,
  PET_ADD_CHANNEL,
  PET_REMOVE_CHANNEL
} from '../shared/ipc.js'
import { createPetLibrary, PetError } from './pet-library.js'
import { isPetRef } from '../shared/avatar.js'
import type { MissionRuntimeId, NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import { isMissionRuntime, runtimeDisplayName } from '../shared/runtimes.js'
import { routeAtStart } from '../shared/route-at-start.js'
import { roleLabelOf } from '../shared/ipc.js'
import { isOutboundLink, isWebLink } from '../shared/outbound-links.js'
import { feedbackUrl, reportFileText, supportMailtoUrl } from './report-problem.js'
import { TESTER_LANE, updateLaneFrom } from './update-lane.js'
import { createCursorConnectorKeeper } from './cursor-connector-allow.js'
import { cursorConfiguredConnectorNames, cursorReadyConnectors, warmCursorConnectors } from './cursor-connector-notice.js'
import { pruneMissionRecords, readStorageReport } from './retention.js'
import { applyPendingRestore, countProfile, readBackup, requestRestore, takeLastRestore, writeBackup, type RestoreOutcome } from './profile-backup.js'
import { oneAtATime } from './one-at-a-time.js'
import { createUpdateService, mayUpdateItself } from './updates.js'
import type {
  CodexMissionCancelRequest,
  DiscoveryEvent,
  CodexMissionStartData,
  CodexMissionStartRequest,
  CodexMissionUpdate,
  MissionMode,
  PublicRoom,
  RoomPost,
  PublicTeammate,
  MissionApprovalRequest,
  MissionApprovalAnswer,
  MissionHandoffRequest,
  MissionResumeRequest,
  SkillLibraryInstallRequest
} from '../shared/ipc.js'
import { ROUTINE_RECOVERY_CHANNEL } from '../shared/routine-recovery.js'
import { decideRoutineRecovery } from './routine-recovery-ipc.js'
import { parsedMissionMode } from './mission-mode.js'
import { createPdfReadings, isPdfPath } from './pdf-reading.js'
import { createViewPdfReader } from './pdf-window.js'
import { createPanelRuns } from './panel-runs.js'
import { PANEL_RUNS_CHANNEL } from '../shared/panel-runs.js'
import { COMMIT_DRAFT_CHANNEL, commitDraftPrompt, draftedCommitMessage, type CommitDraftResponse } from '../shared/commit-draft.js'
import { WALKTHROUGH_CHANNEL, changedPaths, readWalkthrough, walkthroughPrompt, type WalkthroughResponse } from '../shared/walkthrough.js'
import { createViewDiagramDrawer, diagramViewOn } from './diagram-view.js'
import { createDeskHost, keepHandlers, type DeskHost } from './desk-host.js'
import { DESK_COVER_QUERY, DESK_TAKE_BACK_CHANNEL, DESK_WHERE_CHANNEL, PHONE_DESK_CHANNEL } from '../shared/desk.js'
import { keptDiagrams } from './diagrams.js'
import { DIAGRAM_DRAW_CHANNEL } from '../shared/diagram.js'

/*
 * LOCUST IN A BROWSER (0.830, desk-host.ts): every handler the window's calls reach, kept as it is registered, so a
 * browser's call that desk-calls.ts allows reaches the very same one. Kept from here, before any is registered.
 */
const deskHandlers = keepHandlers(ipcMain)
/** Made once the phones' relay link is (below); a window made before then is told nothing more. */
let deskHost: DeskHost | undefined
/** What this computer tells its window, told to a browser that has the window too. */
const mirrorToDesk = (window: BrowserWindow): void => {
  const contents = window.webContents
  const send = contents.send.bind(contents)
  contents.send = (channel: string, ...args: unknown[]): void => {
    send(channel, ...args)
    deskHost?.mirror(channel, args)
  }
}

/*
 * AN ATTACHED PDF IS OPENED FOR THE AGENT (0.713, pdf-reading.ts): its text
 * and page pictures, read in a view no one sees from Locust's own page.
 * Started when the file is attached, so it is usually done by Send.
 */
const pdfReadings = createPdfReadings(
  createViewPdfReader({
    load: async (contents) => {
      if (!app.isPackaged && process.env.ELECTRON_RENDERER_URL) await contents.loadURL(`${process.env.ELECTRON_RENDERER_URL}/pdf.html`)
      else await contents.loadFile(join(__dirname, '../renderer/pdf.html'))
    }
  })
)

/*
 * A DIAGRAM IN A REPLY IS DRAWN (0.770, shared/diagram.ts): in a view no one sees, from Locust's own page, and
 * kept by what it says, so a conversation opened again draws nothing twice.
 */
const drawDiagram = keptDiagrams(
  createViewDiagramDrawer({
    open: diagramViewOn(async (contents) => {
      if (!app.isPackaged && process.env.ELECTRON_RENDERER_URL) await contents.loadURL(`${process.env.ELECTRON_RENDERER_URL}/diagram.html`)
      else await contents.loadFile(join(__dirname, '../renderer/diagram.html'))
    })
  })
)

/** Starts opening the attached PDFs among `paths` (folder-relative), and lets Send wait for it. */
function readAttachedPdfs(folder: string, paths: readonly string[]): void {
  for (const path of paths) {
    if (isPdfPath(path)) void pdfReadings.prepare(folder, path, basename(path)).catch(() => undefined)
  }
}

/**
 * How large a single paste may be.
 *
 * It crosses IPC in one message, so this is a bound on what the renderer can
 * hand the host at once. Generous for a screenshot -- a full 4K PNG is well
 * under it -- and short of anything that would stall a window. Bigger things
 * still go through the + button, which streams from a path rather than
 * carrying bytes.
 */
const MAX_PASTED_BYTES = 24 * 1024 * 1024

const probeRunner = createNodeProbeRunner()
const sessionProbeRunner = createNodeProbeRunner({ maximumTimeoutMs: 30_000, maxOutputBytes: 16 * 1024 * 1024 })
// `claude mcp list` health-checks every connector and asks for 20 s; the
// ordinary runner caps any probe at 10 s, so on a slower machine the list
// was never read and connector allow rules were never made. Its own ceiling,
// so no other probe's default grows with it.
const connectorProbeRunner = createNodeProbeRunner({ maximumTimeoutMs: 30_000 })
// OpenCode's first-start setup (0.780): a 61 MB install, so its own ceiling, as the connector list has.
const openCodeSetUpRunner = createNodeProbeRunner({ maximumTimeoutMs: OPENCODE_SETUP_TIMEOUT_MS })
/** The guard's plugin URL once it is written at start (whenReady); a sweep before then waits for it. */
let markOpenCodeGuard: (url: string | undefined) => void = () => undefined
const openCodeGuardForSetUp = new Promise<string | undefined>((resolve) => { markOpenCodeGuard = resolve })
// `LOCUST_HIDE_RUNTIMES=1` is a test seam: the first-run drive needs a
// machine with nothing installed, and this one has everything.
/*
 * npm's REAL prefix, asked once, so a runtime installed into a moved one is
 * findable.
 *
 * The locator's install-root table names npm's default prefix; a machine that
 * has run `npm config set prefix` -- corporate Windows, any nvm-style manager,
 * or anyone who followed the remedy LOCUST ITSELF prints on an EACCES failure
 * -- puts its shims somewhere else entirely. Discovery then reports "not
 * installed" for something the person has just installed, which is the worst
 * possible answer because it sends them to install it again.
 *
 * Resolved before the locator is built and passed in, so the search order
 * stays PATH first. A machine with no npm answers undefined and nothing
 * changes. See `npm-prefix.ts`.
 */
/*
 * NOT awaited. This was `await readNpmBinDirectory()` at the top of the
 * module, which put `npm config get prefix` -- half a second here, up to its
 * ten-second timeout on a machine where npm hangs -- in front of
 * `app.whenReady` and the splash window. The locator awaits it at the one
 * pass that needs it, after PATH has missed (Fable's probing review, #5).
 */
const npmBinDirectory = readNpmBinDirectory()
/*
 * The Node this app is already made of.
 *
 * An Electron binary started with `ELECTRON_RUN_AS_NODE=1` is a Node
 * runtime -- 24.18.1 in the packaged build, checked by running it. An
 * npm-installed CLI is a `.cmd` shim around a node script, so without a Node
 * on the machine the locator had nothing to run it with and fell back to
 * cmd.exe, which costs the 8,191-character command line.
 *
 * Offered LAST. A Node the person installed is the one their CLIs were
 * built against; this is the answer only when there is no other, and that is
 * the machine Ian had.
 */
const bundledNode = { executablePath: process.execPath, env: { ELECTRON_RUN_AS_NODE: '1' } }
/**
 * And the npm to install those CLIs with, for the same machine. Absent in a
 * build that did not ship one, which is every build before 0.178.0.
 */
const bundledNpmFound = findBundledNpm({
  resourcesPath: process.resourcesPath,
  appPath: app.getAppPath(),
  execPath: process.execPath
})
/*
 * And WHERE it installs, said once and handed to both sides.
 *
 * Grok's pass 10 measured the day-one defect: Install ran, npm said done,
 * and Locust said "installed, but Locust still cannot find the command".
 * npm run by this binary had chosen a prefix from the binary's location and
 * nothing searched it. Now the prefix is a folder in the profile, npm is
 * told to install there, and the locator is told to look there -- from one
 * value, so the two cannot disagree.
 */
// macOS and Linux: the PATH a terminal has, before anything looks for a CLI (shell-path.ts).
if (process.platform === 'darwin' || process.platform === 'linux') process.env.PATH = shellPath(process.platform, process.env.PATH, { shell: process.env.SHELL })
const bundledNpmPrefixPath = bundledNpmPrefix(app.getPath('userData'))
const bundledNpm = bundledNpmFound === undefined ? undefined : { ...bundledNpmFound, prefix: bundledNpmPrefixPath }
const executableLocator = process.env.LOCUST_HIDE_RUNTIMES === '1'
  ? { find: async () => undefined }
  : createPathExecutableLocator({
      bundledNode,
      npmBinDirectory,
      ...(bundledNpm === undefined
        ? {}
        : { ownInstallDirectory: bundledNpmBinDirectory(bundledNpmPrefixPath, process.platform) })
    })
/*
 * What the CLIs on this machine already told us about themselves.
 *
 * Loaded before the first sweep and consulted by discovery for any binary
 * whose files have not changed since -- which skips that runtime's version
 * and help probes entirely. Readiness is still asked every time. Loading it
 * is not awaited here: a sweep that starts before the file has been read
 * simply probes, which is what every sweep did before this existed.
 */
const runtimeFacts = createRuntimeFactsStore({ rootDirectory: app.getPath('userData') })
// Undo a turn (0.674): each folder's kept states, and each turn's record of its way back. In the profile,
// never in the person's folder; not in a profile backup (BACKED_UP_FOLDERS names what is).
// And on a machine with no git, the same Undo from a store of Locust's own (0.780, checkpoints-plain.ts).
const checkpoints = createCheckpoints({ root: join(app.getPath('userData'), 'checkpoints'), withoutGit: createPlainCheckpoints({ root: join(app.getPath('userData'), 'checkpoints') }) })
const keptTurns = createTurnRecords(join(app.getPath('userData'), 'checkpoints', 'turns.json'))
/** Told what each turn wrote (0.840): a routine watching a folder does not start on a teammate's own file. */
let turnWrote: ((workspace: string, paths: readonly string[]) => void) | undefined
const turnRecords: TurnRecords = {
  ...keptTurns,
  async put(runId, record) {
    await keptTurns.put(runId, record)
    void checkpoints.changed(record.workspace, record.before, record.after).then((paths) => turnWrote?.(record.workspace, paths), () => undefined)
  }
}
/** Skills copied for each Claude run outside Auto (0.679); what a crash left is cleared at launch. */
const claudeSkillCopies = join(app.getPath('userData'), 'claude-skills')
/** And for each Codex, OpenCode, Copilot and Antigravity run (0.813, run-skills.ts), cleared the same way. */
const runSkillCopies = join(app.getPath('userData'), 'run-skills')
/** Skills the person kept from a GitHub repository (0.710), handed to every Claude run as the `library` plugin. */
const skillLibrary = createSkillLibrary({ root: join(app.getPath('userData'), 'skill-library') })
const runtimeFactsLoaded = runtimeFacts.load().catch(() => undefined)
// What an ACP agent said it can do at its last run (W12), for Settings > AI agents.
const acpCapabilities = createAcpCapabilitiesStore({ rootDirectory: app.getPath('userData') })
void acpCapabilities.load().catch(() => undefined)
// Antigravity has no CLI probe: its readiness is whether the app is open,
// which the host checks itself and merges into the same sweep.
const antigravityProbe = createAntigravityHostProbe()
/**
 * Checks that have not answered, by agent: the ones the sweep went on without
 * (sweep-settle.ts). Held so that
 *  - a start ON that agent waits for its own check and no other,
 *  - the next sweep joins the check that is going instead of starting a
 *    second `opencode` or `agy` beside it, and
 *  - nothing reads one of these as asked (`startReadiness.swept` skips them).
 */
const outstanding = createOutstandingChecks()
let forgetModelCatalog: () => void = () => undefined

/**
 * An agent the sweep went on without has answered: put its answer where the
 * next read finds it, drop the answers built on its absence, and let the
 * window know through the log it already hears (the `probe.finished` for it
 * was emitted when it landed). One tick later, so it never lands before the
 * sweep's own answer is held.
 */
const answeredLate = (id: string, record: RuntimeDiscovery | undefined): void => {
  setTimeout(() => {
    // Only one check per agent is ever outstanding, so this is that one.
    outstanding.delete(id)
    if (record === undefined) return
    const cached = discoveryCache
    if (cached !== undefined) {
      const there = cached.value.some((entry) => entry.id === id)
      discoveryCache = {
        at: cached.at,
        value: there ? cached.value.map((entry) => (entry.id === id ? record : entry)) : [...cached.value, record]
      }
    }
    startReadiness.swept([record], Date.now())
    runtimeDiscovery.invalidate()
    forgetModelCatalog()
  }, 0)
}

/**
 * Antigravity as a check like the others, announced like everything else: its
 * readiness is whether `agy` answers, asked by the host rather than by a CLI
 * probe, and being outside the swept definitions meant it emitted no events at
 * all. It then appeared in the settled table having never been in the log
 * above it, while Gemini was in the log and not the table (Colin, 2026-09-14).
 * One list, or the two disagree in front of somebody.
 */
const antigravityCheck = (): SweepCheck => {
  const result = (async () => {
    discoveryLog.emit({
      kind: 'probe.started',
      id: 'antigravity',
      bin: 'antigravity',
      product: 'Antigravity',
      at: Date.now()
    })
    const record = await antigravityProbe.discoveryRecord().catch(() => undefined)
    discoveryLog.emit({
      kind: 'probe.finished',
      id: 'antigravity',
      at: Date.now(),
      outcome: record === undefined
        ? 'missing'
        : bootOutcome({
            installed: record.availability === 'available',
            status: record.readiness === 'ready' ? 'ready' : record.readiness === 'authentication-required' ? 'auth-required' : 'other'
          }),
      ...(record?.version?.version === undefined ? {} : { version: record.version.version })
    })
    return record
  })()
  return { id: 'antigravity', result, located: Promise.resolve(), pending: () => antigravityProbe.pendingRecord() }
}

/**
 * Ask the runtimes (all of them, or the named ones) and release the answer
 * when every one has answered or FIRST_SCREEN_DEADLINE_MS has passed
 * (sweep-settle.ts). An agent that is slower is in the answer as installed and
 * still being checked; its own check carries on and its answer is patched in
 * (`answeredLate`). So no single agent -- OpenCode at 4-11 s, Antigravity,
 * whichever is slowest that launch -- holds another agent's row, the first
 * screen, or a Send to a different agent.
 */
const askRuntimes = async (only?: ReadonlySet<string>): Promise<readonly RuntimeDiscovery[]> => {
  // Cheap and bounded: a file read that has already been started.
  await runtimeFactsLoaded
  // A check still going for an agent is joined, not started again.
  const going = outstanding.values().filter((check) => only === undefined || only.has(check.id))
  const goingIds = new Set(going.map((check) => check.id))
  const asking: SweepCheck[] = [...going]
  const mine = new Set<string>()
  const cliOnly = only === undefined ? undefined : new Set([...only].filter((id) => id !== 'antigravity'))
  const cliChecks = cliOnly !== undefined && cliOnly.size === 0
    ? []
    : discoverInstalledRuntimesEach({
        runner: probeRunner,
        locator: executableLocator,
        includeOmniRoute: true,
        recall: runtimeFacts,
        // Only a full sweep is narrated, one row after another.
        ...(only === undefined
          ? {
              /*
               * A beat between starts you can actually SEE.
               *
               * 140ms put all six rows on screen inside 700ms -- less time than the
               * window itself takes to appear, so the log was complete before
               * anybody could watch it happen. Colin, with a photo taken at open:
               * "would be cool if we actually saw the terminal pop all those up."
               *
               * The stagger delays each probe's START, never its finish, and they
               * overlap: the only cost is the last row beginning later than it
               * otherwise would.
               *
               * 240 -> 70 on 2026-09-22, MEASURED: at 240 the sixth row (OpenCode,
               * the slowest real probe at 2.3 s) did not start until 1.2 s into the
               * sweep and set the end of it, and the whole launch took 6.6-7.0 s.
               * Colin, the same day: "i want the users to have a seamless, fast
               * experience". At 70 the rows still arrive one after another, visibly,
               * which is the effect he asked for -- over half a second, not two.
               */
              staggerMs: 70,
              /*
               * `started` reaches the window BEFORE the subprocess is spawned --
               * that is the whole contract. A screen told about the start and the
               * result at the same moment has nothing to draw during the wait, and
               * the wait is the only thing it exists for.
               */
              watch: {
                started: (runtime: { readonly id: string; readonly displayName: string; readonly bin: string }) => {
                  discoveryLog.emit({
                    kind: 'probe.started',
                    id: runtime.id,
                    bin: runtime.bin,
                    product: runtime.displayName,
                    at: Date.now()
                  })
                },
                finished: (runtime: { readonly id: string }, discovery: RuntimeDiscovery) => {
                  const installed = discovery.availability === 'available'
                  discoveryLog.emit({
                    kind: 'probe.finished',
                    id: runtime.id,
                    at: Date.now(),
                    outcome: bootOutcome({
                      installed,
                      status: discovery.readiness === 'ready'
                        ? 'ready'
                        : discovery.readiness === 'authentication-required'
                          ? 'auth-required'
                          : 'other'
                    }),
                    ...(discovery.version?.version === undefined ? {} : { version: discovery.version.version })
                  })
                }
              }
            }
          : {}),
        // Listed but not runnable here: their sign-in answer is never used,
        // so it is not asked for (Gemini's was the slowest probe of a launch).
        readinessFromVersion: plannedRuntimes(),
        ...(cliOnly === undefined ? {} : { only: cliOnly }),
        ...(goingIds.size === 0 ? {} : { skip: goingIds })
      })
  for (const check of cliChecks) {
    asking.push(check)
    mine.add(check.id)
  }
  if ((only === undefined || only.has('antigravity')) && !goingIds.has('antigravity')) {
    asking.push(antigravityCheck())
    mine.add('antigravity')
  }
  const order: readonly string[] = discoveryCache?.value.map((entry) => entry.id) ?? []
  const settled = await settleWithin(asking, {
    deadlineMs: FIRST_SCREEN_DEADLINE_MS,
    late: answeredLate
  })
  for (const check of asking) if (settled.left.has(check.id)) outstanding.set(check)
  // The order the last answer had, so a row does not move between sweeps.
  const rank = (id: string): number => {
    const at = order.indexOf(id)
    return at === -1 ? Number.MAX_SAFE_INTEGER : at
  }
  return [...keepWhatWasAnswered(settled, discoveryCache?.value ?? [])].sort((left, right) => rank(left.id) - rank(right.id))
}
/*
 * OpenCode's first start, done before the first message (0.780, opencode-first-start.ts): once the window finds it
 * ready and its plugin package is not there yet, it sets itself up in the background, once a launch.
 */
const openCodeFirstStart = createOpenCodeFirstStart({
  locate: () => executableLocator.find('opencode'),
  plugin: () => openCodeGuardForSetUp,
  run: (command) => openCodeSetUpRunner.run(command),
  note: (line) => console.warn(line)
})
const discoverRuntimes = (): Promise<readonly RuntimeDiscovery[]> => askRuntimes()
/**
 * The connectors this machine has, read in the background and held.
 *
 * Colin, 2026-09-10: "honestly just let them have access to the mcp tools if
 * the client have access to it -- it only makes sense and is way less muddy."
 * So there is no per-teammate grant: what the person's Claude Code can reach,
 * their teammates can reach.
 *
 * The names are needed because an allow rule must NAME its server -- `mcp__*`
 * is refused outright -- and the account connectors from claude.ai never
 * appear in `~/.claude.json`, so they cannot be read off disk at all.
 * `claude mcp list` is the only source that has all of them, and it is slow
 * because it health-checks each one, so nothing ever waits for it.
 */
const connectorReader = createConnectorReader({
  read: async (timeoutMs) => {
    const claude = await executableLocator.find('claude').catch(() => undefined)
    if (claude === undefined) return undefined
    const result = await connectorProbeRunner.run({
      purpose: 'capabilities',
      executablePath: claude.executablePath,
      args: [...claude.prefixArgs, 'mcp', 'list'],
      timeoutMs,
      // H7: under the app's own Node the launch needs its environment.
      ...(claude.env === undefined ? {} : { env: claude.env })
    })
    // The listing prints on stdout; a machine with none says so there too.
    // Both streams are joined because a warning on stderr has never been a
    // reason to throw the list away.
    // Said in Settings > Connectors (W8): the rules keep the last reading that answered.
    if (result.timedOut === true) return 'timed-out'
    return `${result.stdout}
${result.stderr}`
  }
})

/**
 * One probe sweep, shared.
 *
 * Discovery spawns a version probe and a capability probe per runtime -- ten
 * or more child processes. The cached service below was wired to the UI
 * channel alone, so every mission start, every model-catalog read and every
 * app-server start re-ran the whole sweep, BEFORE any of the checks that
 * might refuse the request. A window could drive that in a loop with a
 * runtime the host was always going to turn down.
 *
 * Ten seconds is the same window the UI already accepts, and a runtime that
 * is installed or signed into mid-session appears on the next read.
 */
const DISCOVERY_TTL_MS = 10_000
let discoveryCache: { readonly at: number; readonly value: readonly RuntimeDiscovery[] } | undefined
let discoveryInFlight: Promise<readonly RuntimeDiscovery[]> | undefined

const discoverForWork = (): Promise<readonly RuntimeDiscovery[]> => {
  const held = discoveryCache
  if (held !== undefined && Date.now() - held.at < DISCOVERY_TTL_MS) return Promise.resolve(held.value)
  if (discoveryInFlight !== undefined) return discoveryInFlight
  /*
   * The sweep announces itself, one probe at a time.
   *
   * Emitted from the ONE place a real sweep happens, so the boot screen can
   * never show a sweep that did not occur, and a cached answer emits nothing
   * -- there is no wait to narrate.
   */
  const running = (async () => {
    await Promise.race([
      waitingForWindow,
      new Promise((resolve) => setTimeout(resolve, WINDOW_WAIT_CAP_MS))
    ])
    discoveryLog.emit({ kind: 'started', at: Date.now() })
    return discoverRuntimes()
  })()
    .then((value) => {
      discoveryCache = { at: Date.now(), value }
      // An agent that is still being checked has not been asked yet, as far as a start can tell.
      startReadiness.swept(value.filter((runtime) => !outstanding.has(runtime.id)), Date.now())
      const ready = value.filter((runtime) => runtime.readiness === 'ready').length
      const needsYou = value.filter((runtime) => runtime.readiness === 'authentication-required').length
      discoveryLog.emit({ kind: 'finished', at: Date.now(), ready, needsYou })
      // Cursor's connectors, read once in the background (0.692): its first turn need not wait 3-4 s for them.
      if (value.some((runtime) => runtime.id === 'cursor' && runtime.readiness === 'ready')) warmCursorConnectors()
      return value
    })
    .finally(() => {
      discoveryInFlight = undefined
    })
  discoveryInFlight = running
  return running
}

/**
 * How long a runtime that was READY is trusted, when the only question is
 * whether a mission may start on it.
 *
 * STARTING ONE MISSION SWEPT ALL EIGHT RUNTIMES. Fable's probing review,
 * 2026-09-21, ranked first and measured: a Send 41 s after launch waited 3 s
 * before the run could begin, because `discover()` re-probes every runtime's
 * version, help, readiness and model list to answer one question about one of
 * them. On Colin's machine the slowest runtime "ran past six seconds". **The
 * Stop button appears at 51 ms**, so the screen says running while the host
 * is busy with six runtimes the mission is not using — which is why nobody
 * ever named this delay despite feeling it every time.
 *
 * Five minutes rather than the sweep's ten seconds, and only for a `ready`
 * record. The reason it is safe is that THE RUN ITSELF IS THE REAL CHECK: a
 * CLI that has been signed out since the last sweep fails at launch, fast and
 * legibly, which is a better answer than making every start wait for a probe
 * that is almost always going to say yes.
 *
 * Anything else — not ready, not available, no record at all — takes the full
 * sweep exactly as before. That is the case where the answer might actually
 * have changed in the person's favour, and it is the one worth waiting for.
 */
const READY_TO_START_TTL_MS = 5 * 60_000

const startReadiness = createStartReadiness({
  ttlMs: READY_TO_START_TTL_MS,
  now: () => Date.now(),
  held: () => discoveryCache,
  // The sweep's own clock is left alone: the other runtimes were not asked.
  store: (value) => {
    discoveryCache = { at: discoveryCache?.at ?? Date.now(), value }
  },
  sweep: discoverForWork,
  askOne: async (runtimeId) => {
    await runtimeFactsLoaded
    const [fresh] = await discoverInstalledRuntimes({
      runner: probeRunner,
      locator: executableLocator,
      includeOmniRoute: true,
      recall: runtimeFacts,
      readinessFromVersion: plannedRuntimes(),
      only: new Set([runtimeId])
    })
    if (fresh !== undefined) discoveryLog.emit({ kind: 'reasked', ids: [runtimeId], at: Date.now() })
    return fresh
  },
  // Not a CLI: its readiness is whether the app is open, asked by the host.
  sweepOnly: new Set(['antigravity'])
})

/**
 * See start-readiness.ts: a stale answer is refreshed for the starting runtime alone.
 *
 * A start ON an agent the sweep went on without waits for THAT agent's own
 * check (sweep-settle.ts) -- the one send that has something to wait for. A
 * start on any other agent, and the model list, never do.
 */
const discoverForStart = async (runtimeId?: string): Promise<readonly RuntimeDiscovery[]> =>
  outstanding.forStart(runtimeId, await startReadiness.forStart(runtimeId))

/**
 * Can npm be run from here?
 *
 * Asked once and remembered: it is a fact about the machine, and asking on
 * every discovery sweep would spawn a child process every ten seconds for an
 * answer that changes when someone installs Node, not between ticks. The
 * remembered answer is dropped whenever an install finishes, which is the one
 * moment it can have changed under us.
 */
let npmSeen: boolean | undefined
/*
 * WHY npm was not found, which is not the same question as whether it was.
 *
 * The probe answers false for three different machines: no Node at all, an
 * npm that errors, and an npm that never answers -- nvm-windows with no
 * version picked, a corporate wrapper waiting on a proxy. All three select
 * the bundled npm, correctly. But the SENTENCE the screen shows for that
 * value says "Node.js is not on this machine", and Fable measured it on a
 * box where Node sat on PATH and only npm hung (pass 2, finding 3). The host
 * knew which of the three had happened and threw it away one line later.
 */
let npmAnswered = true
const NPM_PROBE_TIMEOUT_MS = 5_000
/** Whether the person's own PATH has npm -- what their terminal would run. PATH only: not the app's own npm folders. */
let npmOnPathSeen: boolean | undefined
const npmOnPath = async (): Promise<boolean> => {
  if (npmOnPathSeen !== undefined) return npmOnPathSeen
  npmOnPathSeen = (await createPathExecutableLocator().find('npm').catch(() => undefined)) !== undefined
  return npmOnPathSeen
}

const npmPresent = async (): Promise<boolean> => {
  if (npmSeen !== undefined) return npmSeen
  npmSeen = await new Promise<boolean>((resolve) => {
    // `shell: true` because Windows will not spawn npm.cmd otherwise, and
    // `--version` because it is the cheapest thing npm will answer.
    // Its own group off Windows, so the timeout below can end the shell AND
    // what it started.
    // One command string, not a string and an array: Node joins an array
    // into the shell's line unescaped anyway, and warns that it will stop
    // (DEP0190, in every packaged launch's log until 0.381).
    const probe = spawn('npm --version', { shell: true, windowsHide: true, ...(process.platform === 'win32' ? {} : { detached: true }) })
    /*
     * Bounded, like every CLI probe. It had no timeout, and every discovery
     * answer waited on it: one `npm` shim that never answers -- nvm-windows
     * with no version picked, a corporate wrapper waiting on a proxy -- held
     * the first screen at "checking the runtimes on this machine" for ever,
     * with no rows, no head note and no Install, on the machine that most
     * needs the Install button (Fable, pass 1, finding 3). An npm that does
     * not answer in five seconds is treated as absent, which selects the
     * npm the app carries, which is the path that works without one.
     */
    let settled = false
    const settle = (found: boolean): void => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      resolve(found)
    }
    const timer = setTimeout(() => {
      if (typeof probe.pid === 'number') killSpawnedTree(probe.pid, process.platform)
      // The one branch that means 'it is there and it did not speak'.
      npmAnswered = false
      settle(false)
    }, NPM_PROBE_TIMEOUT_MS)
    probe.on('error', () => settle(false))
    probe.on('close', (code) => settle(code === 0))
  })
  return npmSeen
}

/** Recorded and replayed, so a window created mid-sweep sees the whole log. */
const discoveryLog = createDiscoveryLog()

/*
 * THE FIRST SWEEP WAITS FOR SOMEBODY TO WATCH IT.
 *
 * Something in startup asked for the runtimes about three seconds before
 * the window appeared, so by the time a person could see the boot screen
 * the whole log had already happened and simply arrived at once. Colin,
 * 2026-09-14, with a photo taken a second after launch: "the majority of
 * the text is already appeared on the screen by the time the user opens it
 * so they dont see the whole terminal effect."
 *
 * It is also just wrong on its own terms: six subprocesses should not be
 * spawned before there is a window. Nothing needs the runtimes until
 * something is on screen asking about them.
 *
 * Capped, because a wait with no bound is a hang: a build that never opens
 * a window -- a smoke, a headless drive -- goes ahead after the cap rather
 * than stalling forever.
 */
const WINDOW_WAIT_CAP_MS = 3_000
let windowIsUp: () => void = () => undefined
const waitingForWindow = new Promise<void>((resolve) => {
  windowIsUp = resolve
})

const runtimeDiscovery = createRuntimeDiscoveryService({
  probe: discoverForWork,
  agentCapabilities: (runtime) => acpCapabilities.get(runtime),
  /*
   * The question the first screen asks is "can an Install button run", not
   * "is Node on this machine". Since 0.178.0 those differ: the app carries
   * its own npm and runs it with its own binary as the Node, so a machine
   * with no Node installs fine. Ian's machine was the case that made this
   * matter -- he downloaded Locust and nothing worked until he installed
   * Node, on a screen whose entire job is installing something.
   */
  npmPresent: async () => bundledNpm !== undefined || await npmPresent(),
  // Only when it is the one that WILL run: the installer prefers a machine's
  // own npm and falls back to this, so with Node installed the note would be
  // saying something untrue about the install that is about to happen.
  //
  // AND never while the person's own PATH has npm (0.412). `npmPresent` runs
  // `npm --version` against a five-second clock, at the launch moment every
  // runtime probe runs too; one slow answer read as "no Node" for the whole
  // session, and Colin -- Node in Program Files, npm answering in 0.35s --
  // was told to install Node.js. What his terminal would find is a lookup,
  // not a race: the PATH alone, without the app's own npm folders.
  npmIsBundled: async () => bundledNpm !== undefined && !(await npmPresent()) && !(await npmOnPath()),
  // Said apart, because the sentence differs: an npm that hung is not a
  // machine without Node, and a person who installed Node is told otherwise.
  npmDidNotAnswer: async () => {
    await npmPresent()
    return !npmAnswered
  }
})
const ownsSingleInstanceLock = app.requestSingleInstanceLock()
// What a crashed run left of its skill copies: cleared by the instance that owns the profile only. A second launch,
// about to hand over and quit, cleared them before taking the lock and could pull a live run's files away (Sol's review).
if (ownsSingleInstanceLock) void clearClaudeSkillCopies(claudeSkillCopies)
if (ownsSingleInstanceLock) void clearClaudeSkillCopies(runSkillCopies)
let missionServiceForShutdown: CodexMissionService | undefined
let remoteControlForShutdown: ReturnType<typeof createRemoteControl> | undefined
let terminalForShutdown: ReturnType<typeof createTerminalHost> | undefined
let ownCommandsForShutdown: ReturnType<typeof createOwnCommandHost> | undefined
let browserForShutdown: ReturnType<typeof createBrowserHost> | undefined
let queuedMessagesForShutdown: ReturnType<typeof createQueuedMessageStore> | undefined
/** Antigravity's watches too: never disposed, they polled and wrote on through the flush (B4 lead). */
let antigravityServiceForShutdown: { dispose(): Promise<void> } | undefined
let ledgerForShutdown: MissionLedger | undefined
let workroomForShutdown: Workroom | undefined
let permissionHostForShutdown: { dispose(): Promise<void> } | undefined
let voiceForShutdown: ReturnType<typeof createVoiceHost> | undefined
let mcpForShutdown: ReturnType<typeof createLocustMcpHost> | undefined
let backgroundForShutdown: BackgroundRuns | undefined

/**
 * The renderer names no destinations.
 *
 * `window.open` used to reach `shell.openExternal` for anything with an
 * `https:` or `mailto:` protocol. That is a hole straight through the egress
 * rules this file works to keep: the packaged build cancels every renderer
 * request and refuses every permission, but `openExternal` hands the URL to
 * the operating system, where none of that applies. Anything running in the
 * renderer could have posted the mission ledger to a host of its choosing,
 * one browser launch at a time.
 *
 * Nothing in this shell links out, so nothing is opened. A future feature
 * that needs a link should name the exact URL here, in the host, rather than
 * accept one from the window.
 */


/**
 * Where the remembered window goes. Its own file rather than a key in the
 * roster: losing it costs a window position, and it must never be able to
 * take the roster down with it.
 */
const windowFile = (): string => join(app.getPath('userData'), 'window.json')
// The person's theme (0.789, appearance.ts), read once the app is ready and before the window is made.
const appearanceFile = (): string => join(app.getPath('userData'), 'appearance.json')
let themeChoice: ThemeChoice = 'dark'
const appearanceNow = (): AppearanceState => ({ theme: themeChoice, resolved: resolvedTheme(themeChoice, nativeTheme.shouldUseDarkColors) })
/** Every window is told, and repainted in the new ground, so none shows the old theme behind its page. */
const broadcastAppearance = (): void => {
  const state = appearanceNow()
  for (const target of BrowserWindow.getAllWindows()) {
    if (target.isDestroyed()) continue
    target.setBackgroundColor(windowGround(state.resolved))
    target.webContents.send(APPEARANCE_CHANGED_CHANNEL, state)
  }
}

const readWindowFile = (): SavedWindow | undefined => {
  try {
    return readSavedWindow(JSON.parse(readFileSync(windowFile(), 'utf8')))
  } catch {
    // No file on first launch, and a corrupt one is the same answer: open
    // where a first launch would. Nothing here is worth a startup failure.
    return undefined
  }
}

/**
 * Save the window's shape as the person changes it.
 *
 * Written on a timer rather than per event -- a drag emits `move` for every
 * frame, and writing the file that often would beat the disk for no gain. The
 * final write on close is what makes the last position stick.
 */
const rememberWindow = (window: BrowserWindow): void => {
  let pending: NodeJS.Timeout | undefined

  const save = (): void => {
    if (window.isDestroyed()) return
    // `getNormalBounds` is the un-maximized shape, which is what a maximized
    // window needs to remember: restoring maximized is a flag, and the size
    // underneath it is what un-maximizing goes back to.
    const bounds = window.getNormalBounds()
    const record: SavedWindow = { ...bounds, maximized: window.isMaximized() }
    void writeFile(windowFile(), JSON.stringify(record), 'utf8').catch(() => {
      // A window position is not worth surfacing an error over.
    })
  }

  const later = (): void => {
    if (pending) clearTimeout(pending)
    pending = setTimeout(save, 500)
  }

  window.on('resize', later)
  window.on('move', later)
  window.on('maximize', later)
  window.on('unmaximize', later)
  window.on('close', () => {
    if (pending) clearTimeout(pending)
    save()
  })
}

/*
 * THE LOADING WINDOW.
 *
 * Its own window, shown before the app, closing when the runtimes have
 * answered -- the ordinary shape of an application splash, and what Colin
 * asked for twice (2026-09-14): "have just that monitor screen be the
 * loading screen and once its done, THEN go to our app with its normal
 * splash with all runtimes should be connected."
 *
 * The design handoff argued for the opposite -- the pane, never a
 * full-window takeover -- on the grounds that a takeover "would hide
 * answers to show a decoration". That reasoning was about a relaunch with
 * the app already on screen. Here the app window does not exist yet, so
 * there is nothing being hidden: the cost is honestly a wait before the
 * workspace, which is what a loading screen IS.
 *
 * Sized to the tube, frameless, and never resizable: it is a picture of a
 * monitor, and a monitor you can drag the corner of is a window pretending
 * to be one.
 */
/** Ready but not shown, waiting for the loading window to finish. */
let appWindowWaiting: BrowserWindow | undefined

/** Show the app, whatever became of the splash. Safe to call twice. */
const showAppWindow = (): void => {
  const waiting = appWindowWaiting
  appWindowWaiting = undefined
  if (waiting === undefined || waiting.isDestroyed()) return
  /*
   * Restored, shown and focused, in that order.
   *
   * `show()` alone left the app "opened minimized" (Colin, 2026-09-14): a
   * window held unshown while another window had focus does not
   * necessarily come forward when it is finally shown, and on Windows it
   * can land minimised behind whatever the person was last looking at.
   */
  if (waiting.isMinimized()) waiting.restore()
  if (!waiting.isVisible()) waiting.show()
  waiting.focus()
}

/**
 * SAY WHY A PRELOAD FAILED, not only that it did.
 *
 * Drive records since 2026-09-20 carry "Electron sandboxed_renderer.bundle.js
 * script failed to run" on their first step -- counted 2026-09-23: 4 of 31
 * packaged launches, 0 of 23 from the development build, every one of those
 * runs passing (the bridge was there). The console line has no stack, so
 * nothing says which window or what threw. Electron hands the exception to
 * `preload-error`; it goes to the log with the window it came from.
 */
const reportPreloadErrors = (window: BrowserWindow, which: string): void => {
  window.webContents.on('preload-error', (_event, preloadPath, error) => {
    console.error(`[preload] the ${which} window's preload threw (${preloadPath}):`, error)
  })
}

/** How long the loading window may hold the app before it is opened anyway. */
const SPLASH_CAP_MS = 12_000
const SPLASH_WIDTH = 788
const SPLASH_HEIGHT = 568
let splashWindow: BrowserWindow | undefined

const createSplashWindow = (): BrowserWindow => {
  const splash = new BrowserWindow({
    width: SPLASH_WIDTH,
    height: SPLASH_HEIGHT,
    center: true,
    show: false,
    frame: false,
    resizable: false,
    movable: true,
    minimizable: false,
    maximizable: false,
    skipTaskbar: false,
    /*
     * TRANSPARENT, so the monitor is the only thing on screen.
     *
     * A window with its own ground drew a black slab behind the bezel and
     * around its rounded corners -- Colin, 2026-09-14: "remove the weird
     * black border behind it, just keep our natural border". The bezel
     * already has an edge and a radius of its own; the window should add
     * nothing to it.
     */
    transparent: true,
    // Linux reads a PNG, and from inside the archive (0.789); only Windows needs the .ico beside it.
    icon: app.isPackaged && process.platform === 'win32'
      ? join(process.resourcesPath, 'icon.ico')
      : join(__dirname, '../../resources/icon-512.png'),
    webPreferences: {
      preload: join(__dirname, '../preload/index.cjs'),
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false
    }
  })
  reportPreloadErrors(splash, 'loading')
  splash.once('ready-to-show', () => {
    // Shown AND raised. A loading screen that opens behind the window that
    // had focus is a loading screen nobody sees.
    splash.show()
    splash.focus()
  })
  if (!app.isPackaged && process.env.ELECTRON_RENDERER_URL) {
    void splash.loadURL(`${process.env.ELECTRON_RENDERER_URL}#splash`)
  } else {
    void splash.loadFile(join(__dirname, '../renderer/index.html'), { hash: 'splash' })
  }
  return splash
}

const createWindow = (
  codexMissions: CodexMissionService,
  onWindow: (window: BrowserWindow) => void
): void => {
  const opening = openingPlacement(
    readWindowFile(),
    screen.getAllDisplays().map((display) => display.workArea),
    screen.getPrimaryDisplay().workAreaSize
  )
  const window = new BrowserWindow({
    width: opening.width,
    height: opening.height,
    ...(opening.x === undefined || opening.y === undefined ? {} : { x: opening.x, y: opening.y }),
    // The layout's minimum, or the work area when that is smaller (M21).
    minWidth: opening.minWidth,
    minHeight: opening.minHeight,
    center: opening.x === undefined,
    show: false,
    ...windowChrome(process.platform),
    backgroundColor: windowGround(appearanceNow().resolved),
    // The Locust mark, rasterised by `_tools/render-icon.cjs`. Resolved from
    // the build output, which sits two levels below the app directory both in
    // dev and in the unpackaged build; a packaged build will carry its own
    // `.ico` when packaging exists.
    // The 512, not the 256 beside it. Windows scales the window and taskbar
    // icon from whatever it is handed, so handing it the smaller file made it
    // downsample from a downsample. Found by the design pass, 2026-09-04.
    // Packaged: the .ico copied BESIDE the archive (electron-builder.yml,
    // extraResources), never a path inside it. Two wrong turns, both seen on
    // Colin's taskbar as Electron's own emblem: 0.32.0 handed a PNG inside
    // app.asar, which Windows cannot make a window icon from; 0.33.1 handed
    // nothing, assuming Windows would take the executable's icon -- it does
    // for shortcuts, not for a running window (2026-09-06: "it's literally
    // showing the electron emblem"). Development keeps the PNG.
    // Linux reads a PNG, and from inside the archive (0.789); only Windows needs the .ico beside it.
    icon: app.isPackaged && process.platform === 'win32'
      ? join(process.resourcesPath, 'icon.ico')
      : join(__dirname, '../../resources/icon-512.png'),
    webPreferences: {
      preload: join(__dirname, '../preload/index.cjs'),
      // The theme it starts in, so its first frame is already drawn in it (appearance.ts).
      additionalArguments: [`${THEME_ARGUMENT}${appearanceNow().resolved}`],
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false
    }
  })

  // The default menu would take Ctrl+Shift+I for DevTools, Ctrl+O for Open, and Ctrl+/.
  // Those chords belong to the page. F12 still opens DevTools. Ctrl+W, Ctrl+R and Alt+F4 are not held.
  window.webContents.on('before-input-event', (event, input) => {
    const chord = heldChord({
      type: input.type,
      control: input.control,
      meta: input.meta,
      shift: input.shift,
      alt: input.alt,
      key: input.key,
      code: input.code
    })
    if (chord === undefined) return
    event.preventDefault()
    const detail = JSON.stringify({ key: chord.key, code: chord.code, ctrlKey: true, shiftKey: chord.shift, bubbles: true, cancelable: true })
    void window.webContents.executeJavaScript(`window.dispatchEvent(new KeyboardEvent('keydown', ${detail}))`)
  })

  reportPreloadErrors(window, 'main')
  onWindow(window)

  window.once('ready-to-show', () => {
    // Maximize before showing: maximizing a visible window is a visible jump.
    if (opening.maximized) window.maximize()
    /*
     * HELD until the loading window says it is done.
     *
     * Built, laid out and ready -- just not shown. So when the splash
     * closes, the workspace is already there with its runtimes answered
     * rather than assembling itself in front of somebody.
     *
     * If there is no splash (the preference is off, or it failed to open),
     * this shows at once, which is exactly what it did before.
     */
    if (splashWindow === undefined || splashWindow.isDestroyed()) window.show()
    else appWindowWaiting = window

    const capturePath = app.isPackaged ? undefined : process.env.TEAMMATE_CAPTURE_PATH
    if (capturePath) {
      void (async () => {
        await delay(8_000)
        const image = await window.webContents.capturePage()
        await writeFile(capturePath, image.toPNG())
        app.quit()
      })().catch((error: unknown) => {
        console.error('Failed to capture the Teammate window', error)
        app.exit(1)
      })
    }
  })

  rememberWindow(window)

  // A link a previewed page opens in a new window goes to the person's
  // browser (0.425); nothing else ever opens a window.
  //
  // ASKED FIRST (0.482, R34). A page reads its folder and may send nothing
  // (pageMayReach); an address it opens could carry what it read, and a page
  // can open one on load, unasked. So the person sees the address and
  // chooses, as Claude asks before an artifact's link leaves it. One question
  // at a time: a page that keeps opening windows gets one.
  let askingToOpen = false
  window.webContents.setWindowOpenHandler(({ url, referrer }) => {
    if (!referrer.url.startsWith(`${PAGE_SCHEME}://`) || !/^https?:\/\//i.test(url) || askingToOpen) return { action: 'deny' }
    askingToOpen = true
    void (async () => {
      // A drive's answer, labelled as such: a native dialog cannot be pressed
      // through the page a drive talks to. Read nowhere else.
      const seam = ({ open: 0, cancel: 1 } as Record<string, number>)[process.env.LOCUST_OPEN_LINK_ANSWER ?? '']
      const shown = url.length <= 300 ? url : `${url.slice(0, 300)}… (${String(url.length - 300)} more characters)`
      const response = seam ?? (await dialog.showMessageBox(window, {
        type: 'question',
        buttons: ['Open in your browser', 'Cancel'],
        defaultId: 1,
        cancelId: 1,
        noLink: true,
        title: 'Locust',
        message: 'This page wants to open a website in your browser.',
        detail: `${shown}\n\nAn address can carry what the page read in your folder. Open it only if you expected this link.`
      })).response
      askingToOpen = false
      if (response === 0) await shell.openExternal(url)
    })().catch(() => {
      askingToOpen = false
    })
    return { action: 'deny' }
  })

  window.webContents.on('will-navigate', (event, url) => {
    const activeUrl = window.webContents.getURL()
    if (url !== activeUrl) event.preventDefault()
  })

  window.once('closed', () => {
    // macOS does not quit when the last window closes, and a run whose window
    // is gone can no longer be answered: an approval reaches nobody, so the
    // run cannot finish, cannot be stopped, and pins a ledger record that then
    // refuses to be deleted. This used to need a second call because the
    // approval transport was a service of its own; every mode runs here now,
    // and `interrupt` aborts each live run -- which kills its app-server
    // process -- while `clearActive` refuses whatever it was still asking.
    codexMissions.interrupt()
  })

  if (!app.isPackaged && process.env.ELECTRON_RENDERER_URL) {
    void window.loadURL(process.env.ELECTRON_RENDERER_URL)
  } else {
    void window.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

/**
 * The last thing that must never be silent. An exception nobody caught in
 * the main process used to close the app with no word at all: a beta user
 * would see Locust vanish and not know whether their records survived.
 * They do -- the ledger is append-only and flushed on every write -- so the
 * dialog says so, names the log, and the app goes on unless it cannot.
 */
const errorLog = (): string => join(app.getPath('userData'), 'locust-errors.log')

/**
 * Append one line, rolling the file first if it has grown past its cap.
 *
 * Every failure in here is swallowed on purpose. This runs while something
 * is already going wrong, and a diagnostics file that throws on its way to
 * recording a crash would replace a report with a second crash.
 */
const note = (label: string, detail: string): void => {
  try {
    mkdirSync(app.getPath('userData'), { recursive: true })
    const path = errorLog()
    let bytes: number | undefined
    try {
      bytes = statSync(path).size
    } catch {
      // No file yet, or it cannot be measured. Either way, do not roll.
    }
    if (shouldRoll(bytes)) {
      // One generation. `renameSync` over an existing `.1` replaces it, so
      // the oldest is dropped rather than accumulating for ever.
      try {
        renameSync(path, path + '.1')
      } catch {
        // Could not roll -- appending to an oversized log still beats
        // losing the line.
      }
    }
    appendFileSync(path, diagnosticLine(new Date(), label, detail), 'utf8')
  } catch {
    // Nowhere to write at all.
  }
}

let toldAboutTrouble = false
/*
 * NEVER A BLOCKING DIALOG (QA-2026-09-29 round 2, R25). This was
 * `dialog.showErrorBox`, which is synchronous: it stopped the main process --
 * every teammate's stream, every ledger write -- until OK was pressed, so a
 * foreseeable failure (a CLI that was uninstalled, a folder that was moved)
 * read as the app crashing and froze the rest with it. A rejection the app
 * carries on from is written to the log and nothing more; a real crash is
 * said once, in a dialog that holds nothing up.
 */
const noteTrouble = (label: string, error: unknown, tell: boolean): void => {
  const detail = error instanceof Error ? (error.stack ?? error.message) : String(error)
  note(label, detail)
  if (!tell || toldAboutTrouble) return
  toldAboutTrouble = true
  try {
    void dialog.showMessageBox({
      type: 'error',
      title: 'Locust hit a problem',
      message: 'Something went wrong inside Locust. Your mission records are safe on disk.',
      detail: `${detail.split('\n')[0] ?? ''}\n\nDetails were written to ${errorLog()}.`
    }).catch(() => undefined)
  } catch {
    // Before the app is ready a dialog cannot show; the log has it.
  }
}
process.on('uncaughtException', (error) => noteTrouble('uncaughtException', error, true))
process.on('unhandledRejection', (reason) => noteTrouble('unhandledRejection', reason, false))

/*
 * THE CRASHES THE TWO LINES ABOVE CANNOT SEE.
 *
 * A renderer crash does NOT raise `uncaughtException` in the main process.
 * The window goes blank or disappears and main carries on healthy with
 * nothing to report -- so the single most recognisable way for this app to
 * break, the one a beta tester would describe as "Locust vanished", was the
 * one failure that wrote no line at all.
 *
 * These are `app`-level and so cover every window, including the loading
 * screen, without either of them having to know about this.
 */
app.on('render-process-gone', (_event, _contents, details) => {
  void voiceForShutdown?.cancel()
  // `clean-exit` is a window being closed on purpose; it is not trouble and
  // a log full of it is a log nobody reads.
  if (details.reason === 'clean-exit') return
  note('render-process-gone', describeGone(details))
})
app.on('child-process-gone', (_event, details) => {
  if (details.reason === 'clean-exit') return
  note('child-process-gone', `${details.type} ${describeGone(details)}${details.name === undefined ? '' : ` name=${details.name}`}`)
})

/*
 * A window that stops answering is not dead and gets no dialog: it usually
 * comes back, and Windows draws its own "not responding" on the frame. It IS
 * worth a line, because "it froze for a bit" is otherwise unanswerable.
 */
app.on('web-contents-created', (_event, contents) => {
  contents.on('unresponsive', () => note('unresponsive', 'a window stopped answering'))
  contents.on('responsive', () => note('responsive', 'it started answering again'))
})

/*
 * Native crash dumps, kept on this machine.
 *
 * `uploadToServer: false` is not a default being restated -- it is a promise
 * the app already makes on screen. Settings says "Every mission is recorded
 * to an append-only ledger on this machine. Nothing is uploaded." A crash
 * reporter that phoned home would make that sentence false.
 *
 * Started before `whenReady` because a crash during startup is exactly the
 * one worth catching, and it must be running before there is anything to
 * crash.
 */
crashReporter.start({ uploadToServer: false })

/*
 * The page preview's scheme (0.425, main/page-preview.ts). Registered before
 * the app is ready, as a scheme must be: STANDARD so a page's relative links
 * resolve, SECURE so it is treated as https would be, fetch and streaming so
 * its own scripts and video work.
 */
protocol.registerSchemesAsPrivileged([
  { scheme: PAGE_SCHEME, privileges: { standard: true, secure: true, supportFetchAPI: true, stream: true, corsEnabled: true } }
])


/** Opens the window from the tray when --background started with none. */
let openFromBackground: (() => void) | undefined

/*
 * A LINK THAT OPENS LOCUST WITH A MESSAGE READY (0.770, shared/locust-link.ts). It waits in the inbox until the
 * window takes it, and the window is brought up and told. One that started Locust is in its own arguments
 * (Windows); on macOS it arrives as open-url, maybe before the app is ready; one that arrives while Locust runs
 * comes with a second launch's arguments.
 */
const linkInbox = createLinkInbox()
{
  const launchedWith = linkInArguments(process.argv)
  if (launchedWith !== undefined) linkInbox.arrive(launchedWith)
}
const linkArrived = (link: LocustLink | undefined): void => {
  if (link === undefined) return
  linkInbox.arrive(link)
  if (!app.isReady()) return
  const window = BrowserWindow.getAllWindows()[0]
  if (window === undefined) {
    openFromBackground?.()
    return
  }
  if (window.isMinimized()) window.restore()
  if (!window.isVisible()) window.show()
  window.focus()
  for (const each of BrowserWindow.getAllWindows()) {
    if (!each.isDestroyed() && !each.webContents.isLoading()) each.webContents.send(LOCUST_LINK_WAITING)
  }
}
app.on('open-url', (event, url) => {
  event.preventDefault()
  linkArrived(parseLocustLink(url))
})

if (!ownsSingleInstanceLock) {
  app.quit()
} else {
  app.on('second-instance', (_event, argv) => {
    const link = linkInArguments(argv)
    if (link !== undefined) {
      linkArrived(link)
      return
    }
    const window = BrowserWindow.getAllWindows()[0]
    if (!window) {
      openFromBackground?.()
      return
    }
    if (window.isMinimized()) window.restore()
    // In the tray while teammates work (0.397): opening Locust again brings it back.
    if (!window.isVisible()) window.show()
    window.focus()
  })

  void app.whenReady().then(async () => {
    // A Mac's menu, named Locust rather than the package (mac-menu.ts). Windows and Linux keep theirs.
    if (process.platform === 'darwin') Menu.setApplicationMenu(Menu.buildFromTemplate(macMenuTemplate('Locust')))
    /*
     * A RESTORE ASKED FOR IS APPLIED FIRST (0.614, profile-backup.ts), before
     * anything below reads the profile: a store holding its old state in
     * memory would write it back over the restored file. The window then says
     * what happened, once (PROFILE_LAST_RESTORE_CHANNEL).
     */
    const restored = await applyPendingRestore(app.getPath('userData'), { appVersion: app.getVersion(), now: () => new Date() })
      .catch((error: unknown): RestoreOutcome => ({ ok: false, folder: '', at: new Date().toISOString(), reason: error instanceof Error ? error.message : String(error) }))
    if (restored !== undefined) note('restore', restored.ok ? `from ${restored.folder}; what it replaced is in ${String(restored.aside)}` : `not applied: ${String(restored.reason)}`)
  }).then(() => {
    // A slot at or before this is missed. Taken before the window, so a
    // routine whose time already passed does not start at launch.
    const appOpenedAt = new Date()
    // On Linux (0.789): an AppImage, or a .deb or .rpm the package manager installed (linux-install.ts).
    const linuxInstall = process.platform === 'linux' && app.isPackaged ? linuxInstallOf(process.env, process.resourcesPath) : undefined
    const linuxUpdateTest = linuxUpdateTestOf(linuxInstall, process.env)
    const appVersion = linuxUpdateTest?.version ?? app.getVersion()
    const organizationPolicy = readOrganizationPolicy({ packaged: app.isPackaged, note })
    /*
     * The first line of every run.
     *
     * A log that reaches us with no version in it can only be answered with
     * a question, and by the time we ask, the person has usually updated.
     */
    note('start', startupDetail({
      version: linuxUpdateTest?.version ?? app.getVersion(),
      platform: process.platform,
      release: release(),
      electron: process.versions.electron ?? 'unknown',
      packaged: app.isPackaged
    }))
    // Dark unless the person chose Light or Match the computer (0.789); native menus and dialogs follow it too.
    themeChoice = readTheme(appearanceFile())
    nativeTheme.themeSource = themeChoice
    nativeTheme.on('updated', broadcastAppearance)
    // Windows shows a notification only for an app it can name. Development
    // runs get an id of their own: a dev electron.exe once left a Start-menu
    // shortcut carrying the installed app's id, and Windows drew Electron's
    // atom on every Locust window from then on (see stale-shortcut.ts).
    if (process.platform === 'win32') {
      app.setAppUserModelId(app.isPackaged ? APP_USER_MODEL_ID : DEVELOPMENT_APP_USER_MODEL_ID)
      if (app.isPackaged) {
        // The installed copy takes the Start-menu shortcut back from any
        // other copy that took it (stale-shortcut.ts, 2026-09-24).
        const startMenuLink = join(app.getPath('appData'), 'Microsoft', 'Windows', 'Start Menu', 'Programs', 'Locust.lnk')
        const repaired = repairStartMenuShortcut({
          path: startMenuLink,
          execPath: process.execPath,
          installed: existsSync(join(dirname(process.execPath), 'Uninstall Locust.exe')),
          readShortcut: (path) => {
            if (!existsSync(path)) return undefined
            const link = shell.readShortcutLink(path)
            return { target: link.target, ...(link.appUserModelId === undefined ? {} : { appUserModelId: link.appUserModelId }) }
          },
          writeShortcut: (path, target) => {
            shell.writeShortcutLink(path, 'replace', { target, cwd: dirname(target), appUserModelId: APP_USER_MODEL_ID, icon: target, iconIndex: 0 })
          }
        })
        if (repaired) note('start-menu-repaired', startMenuLink)
        const removed = sweepStaleElectronShortcuts({
          candidates: [join(app.getPath('appData'), 'Microsoft', 'Windows', 'Start Menu', 'Programs', 'Electron.lnk')],
          appId: APP_USER_MODEL_ID,
          readShortcut: (path) => {
            if (!existsSync(path)) return undefined
            const link = shell.readShortcutLink(path)
            return { target: link.target, ...(link.appUserModelId === undefined ? {} : { appUserModelId: link.appUserModelId }) }
          },
          remove: (path) => unlinkSync(path)
        })
        for (const path of removed) console.warn(`Removed a stale Electron shortcut that carried Locust's app id: ${path}`)
      }
    }

    // The renderer gets microphone audio only in its own main frame on Windows
    // and a Mac; cameras, subframes and every other device/web permission
    // remain refused, and packaged builds get no network egress at all (the dev
    // server needs loopback HTTP/WebSocket for Vite and HMR, so that stays dev-only).
    session.defaultSession.setPermissionRequestHandler((contents, permission, callback, details) => {
      callback(voicePermission(microphoneRunsOn(process.platform) && contents !== null && approvalWindow !== undefined && BrowserWindow.fromWebContents(contents) === approvalWindow && details.requestingUrl === contents.getURL(), permission, details.isMainFrame, 'mediaTypes' in details ? details.mediaTypes ?? [] : []))
    })
    session.defaultSession.setPermissionCheckHandler((contents, permission, _origin, details) =>
      voicePermission(microphoneRunsOn(process.platform) && contents !== null && approvalWindow !== undefined && BrowserWindow.fromWebContents(contents) === approvalWindow && details.requestingUrl === contents.getURL(), permission, details.isMainFrame, details.mediaType === undefined ? [] : [details.mediaType]))
    /*
     * THE SPELLCHECKER FETCHES NOTHING (QA-2026-09-29 round 2, R23). Where
     * Electron spellchecks with Hunspell (Linux) it downloads a dictionary
     * from Google on first launch, while Settings says the window makes no
     * requests of its own. Pointed at a folder on this machine instead: a
     * dictionary put there is used, and none is fetched. Windows and macOS
     * spellcheck with the system's own and never download one.
     */
    session.defaultSession.setSpellCheckerDictionaryDownloadURL(`${pathToFileURL(join(app.getPath('userData'), 'dictionaries')).href}/`)
    if (app.isPackaged) {
      session.defaultSession.webRequest.onBeforeRequest(
        { urls: ['http://*/*', 'https://*/*', 'ws://*/*', 'wss://*/*'] },
        (details, respond) => {
          // A web page shown in the preview may load libraries and fonts from
          // the common public hosts and reach nowhere else (0.482, R34), so
          // what it reads of the folder stays here; Locust's own window still
          // loads nothing (0.425).
          respond({ cancel: !(fromPagePreview(details.frame) && pageMayReach(details.url, details.method)) })
        }
      )
    }
    // The folder the teammates work in. The install folder is never it: the
    // Start menu launches the app from there, and until 0.21.5 every teammate
    // on an installed build worked inside AppData\Local\Programs\Locust. With
    // no folder chosen the services still need SOME absolute path to bind,
    // so they get the launch folder -- and every way of starting a run below
    // is refused until a folder is picked, so nothing ever runs in it.
    // `LOCUST_INSTALL_DIR` is a test seam: a development build has no install
    // folder, and the smoke that proves the refusal needs to name one.
    const installDirectory = app.isPackaged
      ? dirname(app.getPath('exe'))
      : process.env.LOCUST_INSTALL_DIR === undefined || process.env.LOCUST_INSTALL_DIR.length === 0
        ? undefined
        : process.env.LOCUST_INSTALL_DIR
    const rememberedWorkspaceFile = join(app.getPath('userData'), 'workspace.json')
    // With nothing chosen, Locust makes a folder rather than refusing the
    // first message (Colin, 2026-09-05). `LOCUST_DEFAULT_WORKSPACE` is the
    // test seam, so a smoke never creates the real Documents\Locust.
    const defaultWorkspace =
      process.env.LOCUST_DEFAULT_WORKSPACE !== undefined && process.env.LOCUST_DEFAULT_WORKSPACE.length > 0
        ? process.env.LOCUST_DEFAULT_WORKSPACE
        : join(app.getPath('documents'), 'Locust')
    const resolved = resolveWorkspacePath({
      argv: process.argv,
      cwd: process.cwd(),
      installDirectory,
      remembered: readRememberedWorkspace(rememberedWorkspaceFile),
      defaultWorkspace,
      platform: process.platform,
      homeDirectory: homedir(),
      systemDirectory: process.env.SystemRoot
    })
    // A default that cannot be made falls back to the old answer: no
    // workspace, every start refused with the reason. Never a crash at boot.
    const workspace = ((): typeof resolved => {
      if (resolved.source !== 'default' || resolved.path === undefined) return resolved
      try {
        mkdirSync(resolved.path, { recursive: true })
        return resolved
      } catch {
        return { path: undefined, source: 'none' }
      }
    })()
    // Changeable while the app runs (0.458): a folder switch no longer restarts it. See switchFolder.
    let workspaceChosen = workspace.path !== undefined
    const workspaceMade = workspace.source === 'default'
    let workspacePath = workspace.path ?? process.cwd()
    const NO_WORKSPACE_MESSAGE =
      'Choose the folder your teammates work in first. Locust was opened from its own install folder, and no teammate should work in there.'
    const noWorkspaceRefusal = () => organizationPolicy.error !== undefined
      ? ({ ok: false, error: { code: 'POLICY_REFUSED', message: organizationPolicy.error } } as const)
      : workspaceChosen ? undefined : ({ ok: false, error: { code: 'NO_WORKSPACE', message: NO_WORKSPACE_MESSAGE } } as const)

    // Named once, because two things need it: the ledger itself, and the
    // reveal root that lets someone open the folder when a write to it fails.
    // A person told "Locust cannot write its ledger" and given no way to go
    // and look at the folder has been informed and not helped.
    const groups = createGroupStore({ rootDirectory: app.getPath('userData') })
    const queuedMessages = createQueuedMessageStore(app.getPath('userData'))
    queuedMessagesForShutdown = queuedMessages
    /*
     * Every folder worked in, by id and path (0.458, folders.ts): a
     * conversation from another folder is listed, opened and continued there.
     */
    const folders = createFolderRegistry({ file: join(app.getPath('userData'), 'folders.json') })
    /*
     * A Locust install is never a folder to work in (0.458): this one's, or any
     * other copy's (it holds resources/app.asar). Early builds ran conversations
     * in the install folder, so Colin's history names one.
     */
    const isInstallFolder = (path: string): boolean =>
      isInsideDirectory(path, installDirectory, process.platform) || existsSync(join(path, 'resources', 'app.asar'))
    /** A folder's path by id, never an install folder's: a reply there is refused, not run inside Locust. */
    const workableFolderOf = async (id: string): Promise<string | undefined> => {
      const path = await folders.pathOf(id)
      return path === undefined || isInstallFolder(path) ? undefined : path
    }
    if (workspaceChosen) {
      workspacePath = sameFolderAtStart(join(app.getPath('userData'), 'folders.json'), workspacePath)
      void folders.use(workspacePath).catch(() => undefined)
    }
    /*
     * SWITCH FOLDERS WITHOUT A RESTART (0.458). It was `app.relaunch()`: the
     * window vanished and a new one opened, which read as a crash (9/11,
     * 9/29), and the new window listed only that folder's conversations, which
     * read as the team being reset. Now the folder is simply the window's
     * current one: new conversations start there; every conversation keeps
     * its own folder (codex-mission.ts, runFolder).
     */
    const switchFolder = async (next: string): Promise<FolderContext> => {
      const full = await folders.sameAs(resolvePath(next)).catch(() => resolvePath(next))
      await writeRememberedWorkspace(rememberedWorkspaceFile, full)
      workspacePath = full
      workspaceChosen = true
      memoryWorkspaceId = workspaceIdFor(full)
      memoryWorkspaceName = basename(full) || full
      await folders.use(full)
      return { path: full, id: memoryWorkspaceId, name: memoryWorkspaceName }
    }
    /*
     * FOLDERS LIKE HOME OR .claude, SAID YES TO (0.790, workspace.ts startHereCaution): the person is asked once per
     * folder, in the picker or before the first conversation there, and the answer is kept here.
     */
    const folderCautionsFile = join(app.getPath('userData'), 'folder-cautions.json')
    let confirmedFolders = readConfirmedFolders(folderCautionsFile)
    const folderCaution = (folder: string): string | undefined =>
      notATeammateFolder(folder, app.getPath('home'), process.platform, process.env.SystemRoot)
    const folderConfirmed = (folder: string): boolean => confirmedFolders.some((kept) => sameFolder(kept, folder, process.platform))
    const confirmFolder = async (folder: string): Promise<void> => {
      if (folderConfirmed(folder)) return
      confirmedFolders = [...confirmedFolders, resolvePath(folder)]
      // Not kept means asked again next time, which is the safe way round.
      await writeConfirmedFolders(folderCautionsFile, confirmedFolders).catch(() => undefined)
    }
    /*
     * A folder id with no known path, from before folders were kept: found in
     * the conversation's own record (recoverFolderPath). Tried once per id per
     * launch, whatever the answer.
     */
    const triedFolderIds = new Set<string>()
    const recoverFolders = async (missions: readonly { readonly missionId: string; readonly workspaceId: string }[]): Promise<void> => {
      const known = new Set((await folders.list()).map((folder) => folder.id))
      const found: { id: string; path: string }[] = []
      for (const mission of missions) {
        if (known.has(mission.workspaceId) || triedFolderIds.has(mission.workspaceId) || !/^ws_[0-9a-f]{32}$/.test(mission.workspaceId)) continue
        if (!/^mission_[A-Za-z0-9-]{1,80}$/.test(mission.missionId)) continue
        const text = await readFile(join(ledgerDirectory, `${mission.missionId}.jsonl`), 'utf8').catch(() => '')
        const path = recoverFolderPath(mission.workspaceId, text)
        if (path !== undefined) {
          found.push({ id: mission.workspaceId, path })
          known.add(mission.workspaceId)
          triedFolderIds.add(mission.workspaceId)
        }
      }
      // An id whose conversations all failed to say is not tried again this launch.
      for (const mission of missions) if (!known.has(mission.workspaceId)) triedFolderIds.add(mission.workspaceId)
      await folders.learn(found)
    }
    const ledgerDirectory = join(app.getPath('userData'), 'mission-ledger')
    // Counted, so a reader of the whole history can tell nothing changed (the phone page, P4).
    const { ledger: missionLedger, writes: ledgerWrites } = countingLedgerWrites(createFileMissionLedger({ rootDirectory: ledgerDirectory }))
    /** From a turn back to its conversation's root, one ledger read per hop, remembered. */
    const conversationChain = createConversationChain(missionLedger)
    // Its own directory: the ledger treats every `.jsonl` in ITS directory as
    // a mission, and the channel is not one.
    // Team memory: kept per folder in the app's own data, briefed to every
    // teammate mission, written by a reply's memory block. Colin's call
    // (2026-09-05): the shared memory Claude Code and Cursor keep, managed
    // from the app -- so every memory names who, where and from what.
    const memories = createMemoryStore({ rootDirectory: app.getPath('userData') })
    /*
     * The person's own models (0.357): their company's endpoint, or one on
     * this machine, run through OpenCode. A key is kept only as the
     * operating system encrypts it (safeStorage: DPAPI on Windows).
     */
    const ownModels = createOwnModelStore({
      rootDirectory: app.getPath('userData'),
      secrets: {
        available: () => safeStorage.isEncryptionAvailable(),
        encrypt: (text) => safeStorage.encryptString(text),
        decrypt: (cipher) => safeStorage.decryptString(cipher)
      },
      // Priced from OpenCode's own copy of models.dev's list (0.810), where the model is its provider's own.
      prices: createPriceList({ path: openCodePriceListPath(process.env, homedir()) })
    })
    // Each teammate's own skills (0.811), kept from its tunes: handed to its Claude Code runs as the `teammate` plugin.
    const teammateSkills = createTeammateSkillStore({ rootDirectory: app.getPath('userData') })
    // Keys a teammate needs (0.833, teammate-keys.ts): by name to the window, by value only to that teammate's runs.
    const teammateKeys = createTeammateKeys({
      directory: app.getPath('userData'),
      secrets: { available: () => safeStorage.isEncryptionAvailable(), encrypt: (text) => safeStorage.encryptString(text), decrypt: (cipher) => safeStorage.decryptString(cipher) }
    })
    let memoryWorkspaceId = workspaceChosen ? workspaceIdFor(workspacePath) : 'ws_none'
    let memoryWorkspaceName = workspaceChosen ? basename(workspacePath) || workspacePath : 'no folder'
    /** A folder by path, with the id and name memory and the brief know it by (0.458). */
    const folderContext = (path: string): FolderContext => ({ path, id: workspaceIdFor(path), name: basename(path) || path })
    // The folder's own LOCUST.md rides in the same slot, first: read fresh at
    // every start so an edit lands on the next mission (parity row 45).
    const memoryBriefing: MemoryBriefing = {
      section: async (peer, conversation, query, runtime) => {
        if (!workspaceChosen && conversation?.folder === undefined) return undefined
        // The run's own folder: its conversation's, which may not be the window's (0.458).
        const here = folderContext(conversation?.folder ?? workspacePath)
        const sections: string[] = []
        const brief = await readWorkspaceBrief(here.path, undefined, runtime).catch(() => undefined)
        // A teammate with a worktree is not standing in the folder that
        // name belongs to, and saying otherwise sends it looking.
        // `peer` is absent for a run that belongs to nobody. It stands in the
        // project folder, like any run with no worktree, so every branch
        // below reads an absent peer as "no worktree of its own".
        /*
         * A run that stands apart from the project folder: a teammate's
         * worktree, or a comparison column's copy (0.638). Either is told it
         * has its own copy and never the project folder's name -- an arena
         * round's OpenCode columns, told "You are working in the folder
         * arena-rpg" from inside a copy, made that folder there and wrote
         * the game into it.
         */
        const standsApart = peer?.cwd !== undefined || conversation?.ownFolder !== undefined
        /*
         * And told where it stands even when there is no LOCUST.md.
         *
         * 0.36.4 fixed worktree runs dying on a directory refusal partly with
         * one sentence -- "you have your own copy, work only inside the
         * folder you were started in" -- which lived inside the brief and so
         * reached only a folder with a LOCUST.md (QA, 2026-09-06). And a
         * teammate in the FOLDER with no LOCUST.md was told nothing about
         * which folder -- a Cursor teammate dogfooding the app on 2026-09-15
         * had been editing the wrong one. `folderSentences` says each case
         * once (workspace-brief.ts).
         */
        // By its full path (0.640): the name alone was added to the working folder by a small model.
        sections.push(...folderSentences(brief, standsApart, here.path))
        // Word and PowerPoint words as text, readable in any mode (0.530, office-words.ts).
        const officeWords = await officeWordsSection(peer?.cwd ?? here.path).catch(() => undefined)
        if (officeWords !== undefined) sections.push(officeWords)
        /*
         * The group's standing instructions, after the folder's and before
         * memory. Only when this conversation is in a group that has some:
         * the sidebar says nothing about instructions being in effect, and
         * this is the one place that makes such a sentence true.
         *
         * A groups file that will not read briefs nothing, the same as no
         * group -- a turn is never refused over a brief it could do without.
         */
        const listedGroups = await groups.list().catch(() => undefined)
        if (listedGroups !== undefined) {
          const inGroup = groupBriefFor(await conversationChain.keysBefore(conversation?.previousMissionId), listedGroups)
          if (inGroup !== undefined) sections.push(groupSection(inGroup.name, inGroup.instructions))
        }
        // This teammate's own instructions (0.706), after the group's: the
        // narrower word comes later, as the folder's comes before the group's.
        const own = peer === undefined ? undefined : (await teammates.list().catch(() => [])).find((entry) => entry.teammateId === peer.self.teammateId)
        const instructions = columnInstructions(own?.instructions, conversation?.instructions)
        if (own !== undefined && instructions !== undefined && instructions.length > 0) sections.push(teammateSection(own.name, instructions))
        // Its keys by name, never by value, and how to ask for one it needs (0.833).
        if (own !== undefined) {
          const keys = keysSection(await teammateKeys.names(own.teammateId).catch(() => []), runtime !== undefined && KEYS_REACH.has(runtime))
          if (keys.length > 0) sections.push(keys)
        }
        const settings = await teammates.readSettings()
        // About you (0.423): the person's own standing note, before memory
        // and whatever the memory mode says -- switching teammate memory off
        // does not silence the person (shared/about-you.ts).
        if (settings.aboutYou !== undefined) sections.push(aboutYouSection(settings.aboutYou))
        if (settings.memoryMode !== 'off') {
          sections.push(await memoryPart(peer, query, here))
        } else {
          // Off: the file a run could still read says so, rather than
          // holding the last memories (A1.7).
          await retireMemoryFile(peer?.cwd ?? here.path).catch((error: unknown) => {
            note('memory-file', `could not retire ${MEMORY_FILE} under ${peer?.cwd ?? here.path}: ${error instanceof Error ? error.message : String(error)}`)
          })
        }
        return sections.length === 0 ? undefined : sections.join('\n\n')
      }
    }
    /*
     * Memory recall by meaning (memory-recall.ts): the model beside app.asar
     * once packaged, in resources/recall in development (staged by
     * _tools/vendor-recall.mjs). Loaded on the first brief that has a
     * question to rank by, never at launch.
     */
    const memoryRecall = createMemoryRecall({
      directory: app.isPackaged ? join(process.resourcesPath, 'recall') : join(__dirname, '../../resources/recall'),
      cacheFile: join(app.getPath('userData'), 'memory-recall-cache.json'),
      note: (message) => note('memory-recall', message)
    })
    async function recallSimilarity(query: string | undefined, texts: readonly string[]): Promise<{ similarity?: readonly number[] }> {
      if (query === undefined) return {}
      const similarity = await memoryRecall.similarity(query, texts)
      return similarity === undefined ? {} : { similarity }
    }
    async function memoryPart(peer: MissionPeerContext | undefined, query: string | undefined, here: FolderContext): Promise<string> {
        const settings = await teammates.readSettings()
        // An unreadable memory file refuses to be read now rather than
        // passing for empty (memory-store.ts); the run still starts, told
        // plainly, and the file is left exactly as it was.
        let listed: Awaited<ReturnType<typeof memories.briefed>>
        try {
          // Ordered and dated by the last write: a rewritten named memory is today's (shared/memory.ts lastWritten).
          listed = byLastWritten(await memories.briefed(here.id))
        } catch (error) {
          note('memory', `the memory file could not be read for a brief: ${error instanceof Error ? error.message : String(error)}`)
          return 'TEAM MEMORY could not be read for this run. Nothing in it was changed; do not assume it is empty.'
        }
        // A1.3: which named files moved on since each memory was written,
        // checked in the folder the run stands in.
        const standsIn = peer?.cwd ?? here.path
        const changed = await Promise.all(listed.map((memory) => changedSince(standsIn, memory.text, lastWritten(memory)).catch(() => [])))
        // A1.4: which have gone a month without being given to anyone.
        const trackingSince = await memories.briefTrackingSince().catch(() => undefined)
        const briefedAt = new Date()
        const lines = listed.map((memory, index) => ({
          id: memory.memoryId,
          ...(changed[index]!.length === 0 ? {} : { changedSince: changed[index]! }),
          ...(daysUnused(memory, trackingSince, briefedAt) === undefined ? {} : { unusedDays: daysUnused(memory, trackingSince, briefedAt)! }),
          text: memory.text,
          scope: memory.scope,
          // Whoever wrote the words the teammate will read (A1.6).
          by: (memory.updatedBy ?? memory.by).name,
          where: memory.scope === 'global' && memory.workspaceId !== here.id ? memory.workspaceName : undefined,
          at: lastWritten(memory)
        }))
        // The whole list as a file where the run stands, so the brief can
        // paste the newest few and the teammate can read the rest itself.
        // A write that fails used to vanish here -- Grok's pass 12, on Linux:
        // a line typed on the Memory screen, no .locust/memory.md, and no
        // record anywhere of why. The brief still pastes the lines; the
        // failure goes to the error log so the next report can name it.
        const file = await writeMemoryFile(peer?.cwd ?? here.path, lines, new Date()).catch((error: unknown) => {
          note('memory-file', `could not write ${MEMORY_FILE} under ${peer?.cwd ?? here.path}: ${error instanceof Error ? error.message : String(error)}`)
          return undefined
        })
        // `writeMemoryFile` answers undefined for a failed write rather than
        // throwing, so the catch above never ran and 0.184.0's promised log
        // line could not be written (Fable, pass 1, from reading). Logged on
        // the answer, which is the one signal the write gives.
        if (file === undefined) {
          note('memory-file', `could not write ${MEMORY_FILE} under ${peer?.cwd ?? here.path}: the write did not complete (see memory-file.ts)`)
        }
        const sectionInput = {
          ...(file === undefined ? {} : { file }),
          ...(query === undefined ? {} : { query }),
          ...(peer === undefined ? {} : { selfName: peer.self.name }),
          // Undefined for a worktree teammate, for the same reason the brief
          // above stopped naming it: memory's own line said "what is
          // remembered for the folder <parent>", which names the folder the
          // run is NOT standing in -- the exact invitation 0.36.4 removed
          // from the other section and left here (QA, 2026-09-06). Memory is
          // the project's either way; only the pointer at a folder goes.
          workspaceName: peer?.cwd === undefined ? here.name : undefined,
          memories: listed.map((memory, index) => ({
            id: memory.memoryId,
            text: memory.text,
            ...(changed[index]!.length === 0 ? {} : { changedSince: changed[index]! }),
            scope: memory.scope,
            by: (memory.updatedBy ?? memory.by).name,
            where: memory.scope === 'global' && memory.workspaceId !== here.id ? memory.workspaceName : undefined,
            // So a teammate can tell a note from this morning from one that
            // has been sitting there since August.
            at: lastWritten(memory)
          })),
          askFirst: settings.memoryMode === 'ask',
          // Its section, just before this one, teaches the `about you ::` line (shared/memory.ts).
          ...(settings.aboutYou === undefined ? {} : { aboutYouGiven: true }),
          // By meaning when this machine's recall answers in time; the
          // keyword order otherwise (memory-recall.ts).
          ...(await recallSimilarity(query, listed.map((memory) => memory.text)))
        }
        /*
         * A1.4: what this teammate is GIVEN is exactly what the section
         * pastes -- the same choice, made by the same function -- and that
         * is what is noted, at most once a day per memory.
         */
        const given = briefedMemories(sectionInput).flatMap((line) => (line.id === undefined ? [] : [line.id]))
        void memories.noteBriefed(given).catch((error: unknown) => {
          note('memory', `could not note which memories were briefed: ${error instanceof Error ? error.message : String(error)}`)
        })
        return memorySection(sectionInput)
    }
    const workroom = createFileWorkroom({
      rootDirectory: join(app.getPath('userData'), 'workroom')
    })
    // Bound late: the relay starts runs through the service that calls it.
    let relay: Relay | undefined
    let routineRunner: RoutineRunner | undefined
    // The newest step each routine started (0.704): the MCP server's routine_status reads its answer from it.
    const routineLastStep = new Map<string, string>()
    let memoryReader: MemoryReader | undefined
    let attentionReader: AttentionReader | undefined
    // 0.774: a turn whose agent's plan ran out, carried on (carry-on.ts). It starts runs through the service too.
    let carryOn: CarryOn | undefined
    /*
     * FIX UNTIL THE CHECK PASSES (0.812, check-fix.ts): a failed check sent back to the teammate, when the person turned
     * that on for the folder. Made once the start path exists, further down; the mission service calls it after a check.
     */
    let checkFix: CheckFix | undefined
    /**
     * The route each turn started on, as it was resolved -- model, level, account and mode -- so a fix runs exactly as
     * the turn it fixes did, even where the person picked a route for that one message. Kept for this session only.
     */
    const routeOfTurn = new Map<string, { readonly runtime: MissionRuntimeId; readonly model?: string; readonly effort?: string; readonly account?: string; readonly mode: MissionMode }>()
    const rememberRouteOfTurn = (missionId: string, route: { readonly runtime: MissionRuntimeId; readonly model?: string; readonly effort?: string; readonly account?: string; readonly mode: MissionMode }): void => {
      routeOfTurn.set(missionId, route)
      while (routeOfTurn.size > 500) {
        const oldest = routeOfTurn.keys().next().value
        if (oldest === undefined) break
        routeOfTurn.delete(oldest)
      }
    }
    // Bound late for the same reason: it reads the ledger the service writes.
    let roomTasks: RoomTasks | undefined
    /**
     * Raise the approval card, and tell the OS by name if the person is
     * looking elsewhere. Shared: Codex's app-server approvals and Claude
     * Code's permission host are the same card, answered the same way.
     */
    /*
     * SAVED APPROVAL RULES (0.521, shared/approval-rules.ts). Before a card
     * reaches the person, the rules they saved are read: a deny or an allow
     * that covers it exactly answers it through the same decide the card
     * would have used, and the conversation says which rule did. Anything
     * else -- no rule, a question, a compound command, a store that cannot
     * be read -- reaches the person as before.
     */
    /*
     * The connector rules Cursor runs are given (cursor-connector-allow.ts),
     * with what is held written to the profile (0.588): a run live when Locust
     * quit never gave its rules back, and the next start found rules it did
     * not add. Recovered here, before any run can start.
     */
    const cursorHoldsFile = join(app.getPath('userData'), 'cursor-connector-holds.json')
    const cursorConnectors = createCursorConnectorKeeper(undefined, {
      read: () => readFile(cursorHoldsFile, 'utf8').catch(() => undefined),
      write: (text) => writeFile(cursorHoldsFile, text, 'utf8')
    })
    void cursorConnectors.recover().then((changed) => {
      if (changed.length > 0) note('connectors-recovered', `took back the rules an earlier run left in ${changed.join(', ')}`)
    }).catch((error: unknown) => note('connectors-recovered', `could not take back earlier rules: ${error instanceof Error ? error.message : String(error)}`))
    const approvalRules = createApprovalRuleStore({ rootDirectory: app.getPath('userData') })
    /*
     * THE ONE WAY AN APPROVAL IS ANSWERED (A2.16, widened 0.521). Whichever
     * host holds the id answers it. Called from exactly three places, each a
     * decision of the person's: their click on the card (the decide handler),
     * their click on "Yes, and don't ask again" / "Never allow this" (the
     * from-card handler), and the one decision path (answerByDecision, 0.616),
     * which answers by a rule they saved or by their own Always on an earlier
     * card of the run. Nothing a teammate writes reaches it; the guard test
     * (approvals-come-only-from-the-window) holds that.
     */
    const answerApproval = async (answer: MissionApprovalAnswer, by: MissionApproval['by'] = 'card', words?: string): Promise<boolean> => {
      const answered = codexMissions.decide(answer) || permissionHost.decide(answer) || (await antigravityMissions.decide(answer))
      if (answered) {
        recordAnswer(answer, by, words)
        // Recorded: the card is no longer waiting (0.599), so the cap below counts only open cards.
        raisedApprovals.forget(answer.approvalId)
        // The card's message in a chat app (0.831, 0.832) says how it was answered, and loses its buttons.
        for (const link of chatLinks) link.cardSettled(answer.approvalId, 'answers' in answer ? 'answered' : answer.decision === 'deny' ? 'denied' : 'allowed')
      }
      return answered
    }
    // Your teammates in chat apps (0.831 Telegram, 0.832 Discord and Slack; chat-link.ts): made with the phone's calls further down.
    let chatLinks: readonly ChatLink[] = []
    const raisedApprovals = createRaisedApprovals()
    /** A card nothing waits on any more comes down everywhere it shows: the window, the phone's list, a chat app (0.837). */
    const withdrawCard = (approvalId: string): void => {
      const target = approvalWindow
      if (target && !target.isDestroyed() && !target.webContents.isDestroyed()) target.webContents.send(MISSION_APPROVAL_WITHDRAWN_CHANNEL, approvalId)
      raisedApprovals.forget(approvalId)
      for (const link of chatLinks) link.cardSettled(approvalId, 'gone')
    }
    // A card's answer the ledger refused: tried again at the run's end, else said in the record (0.587).
    const unwrittenAnswers = createUnwrittenAnswers({ ledger: missionLedger })
    /*
     * EVERY ANSWER IS WRITTEN DOWN (0.576, ledger v20). Saving a conversation's
     * record found the ledger held no approval: a declined call kept its own
     * words, and an allowed one left no trace, so the record could not show
     * what the person let a teammate do. Written here, where every answer
     * passes, with the card as it was raised; never awaited -- a slow disk
     * must not hold the run the answer releases -- and a mission from before
     * v20 simply cannot take it.
     */
    const recordAnswer = (answer: MissionApprovalAnswer, by: MissionApproval['by'], words: string | undefined): void => {
      const raised = raisedApprovals.get(answer.approvalId)
      if (raised === undefined) return
      const { request } = raised
      const asked = [request.summary, request.detail].map((text) => text.trim()).filter((text, index, all) => text.length > 0 && all.indexOf(text) === index).join('\n')
      const given = 'answers' in answer
        ? 'answered'
        : answer.decision === 'deny'
          ? 'denied'
          : answer.decision === 'approve-always'
            ? 'allowed-always'
            : 'allowed'
      const said = words
        ?? ('answers' in answer ? Object.values(answer.answers).flat().join('; ') : answer.decision === 'deny' ? answer.reason : undefined)
      const approval = boundedApproval({
        approvalId: request.approvalId,
        kind: request.kind,
        asked,
        answer: given,
        by,
        ...(said === undefined || said.trim().length === 0 ? {} : { words: said }),
        askedAt: request.requestedAt,
        occurredAt: new Date().toISOString()
      })
      void missionLedger
        .appendApproval(request.missionId, approval)
        // Not swallowed (0.587, QA's Q2): tried again when the run ends, and
        // still refused, said in the record (approval-record-note.ts).
        .catch((error: unknown) => unwrittenAnswers.remember(request.missionId, approval, error instanceof Error ? error.message : String(error)))
    }
    const ruleContextOf = async (request: MissionApprovalRequest): Promise<{ teammateId?: string; folder?: string }> => {
      const owners = await teammates.missionOwners().catch(() => ({}) as Record<string, string>)
      const teammateId = owners[request.missionId]
      return { ...(teammateId === undefined ? {} : { teammateId }), ...(request.cwd === null ? {} : { folder: request.cwd }) }
    }
    // The Always answers of each run, under the host's key for each (0.616).
    const runAlways = createRunAlways()
    /*
     * ONE DECISION PATH (0.616, the PRD's R8; shared/who-decides.ts). Every
     * request every host raises is decided here before a card is drawn: a
     * saved rule first, no before yes; then an Always given earlier in this
     * run, never for a command that reaches other programs; then the card.
     * Rules that cannot be read decide nothing, an earlier Always included:
     * a rule saying no might be among them, so the card asks.
     */
    const answerByDecision = async (request: MissionApprovalRequest): Promise<{ readonly answered: boolean; readonly askedAgain?: string }> => {
      const rules = await approvalRules.list().catch(() => undefined)
      if (rules === undefined) return { answered: false }
      const context = await ruleContextOf(request)
      const remembered = request.alwaysKey !== undefined && runAlways.has(request.runId, request.alwaysKey)
      const decision = decide(request, { ...context, rules, remembered })
      if (decision.verdict === 'ask') return { answered: false, ...(decision.why === undefined ? {} : { askedAgain: decision.why }) }
      // After the runtime has finished registering the request it just raised.
      await new Promise((settle) => setTimeout(settle, 0))
      if (decision.by === 'earlier-always') {
        // In the record, not the thread: the person chose this on a card already.
        const words = request.alwaysCovers === undefined ? undefined : `The Always given earlier in this run allows ${request.alwaysCovers}.`
        return { answered: await answerApproval({ approvalId: request.approvalId, decision: 'approve-once' }, 'earlier-always', words) }
      }
      const roster = context.teammateId === undefined ? [] : await teammates.list().catch(() => [])
      const sentence = ruleSentence(decision.rule, roster.find((entry) => entry.teammateId === context.teammateId)?.name)
      const answer = decision.verdict === 'allow'
        ? { approvalId: request.approvalId, decision: 'approve-once' as const }
        : { approvalId: request.approvalId, decision: 'deny' as const, reason: `A rule the person saved says no: ${sentence}` }
      const answered = await answerApproval(answer, 'saved-rule', sentence)
      if (!answered) return { answered: false }
      await approvalRules.used(decision.rule.ruleId).catch(() => undefined)
      sendToWindow({
        kind: 'relay-notice',
        runId: request.runId,
        missionId: request.missionId,
        // The rule's own sentence names what it covered; Settings is where it is undone.
        message: `${decision.verdict === 'allow' ? 'Allowed' : 'Denied'} by your saved rule: ${sentence} Settings > Teammates lists your rules.`
      })
      return { answered: true }
    }
    const raiseApproval = (request: MissionApprovalRequest): void => {
      void (async () => {
        const context = await ruleContextOf(request).catch(() => ({}) as { teammateId?: string })
        raisedApprovals.remember(request, context.teammateId)
        const decided: { readonly answered: boolean; readonly askedAgain?: string } = await answerByDecision(request).catch(() => ({ answered: false }))
        if (decided.answered) return
        // A card shown although an Always was given says why (a command that reaches other programs).
        showApproval(decided.askedAgain === undefined ? request : { ...request, askedAgain: decided.askedAgain })
      })()
    }
    const showApproval = (request: MissionApprovalRequest): void => {
        const target = approvalWindow
        if (target && !target.isDestroyed() && !target.webContents.isDestroyed()) {
          target.webContents.send(MISSION_APPROVAL_CHANNEL, request)
        }
        // A run stopped waiting on a person who is looking elsewhere is told
        // through the OS, by name, and the click brings the window back.
        void teammates
          .missionOwners()
          .then(async (owners) => {
            const ownerId = owners[request.missionId]
            const roster = ownerId === undefined ? [] : await teammates.list()
            const name = roster.find((entry) => entry.teammateId === ownerId)?.name
            attention.approvalArrived(request, name)
            phoneAlerts.approval(name, request.kind === 'question' ? 'question' : 'approval', request.approvalId)
            // A turn started in a chat app brings its card to the chat too (0.831, 0.832).
            for (const link of chatLinks) link.cardRaised(request, name)
          })
          .catch(() => { attention.approvalArrived(request, undefined); phoneAlerts.approval(undefined, request.kind === 'question' ? 'question' : 'approval', request.approvalId); for (const link of chatLinks) link.cardRaised(request, undefined) })
      }

    /*
     * Locust as Claude Code's permission host. See permission-host.ts for
     * what was measured. The bridge script ships BESIDE app.asar, because
     * Claude Code spawns it as a process and cannot spawn a path inside
     * an archive -- the same reason the window icon lives there.
     */
    const permissionHost = createPermissionHost({
      bridgePath: app.isPackaged
        ? join(process.resourcesPath, 'locust-permission-bridge.mjs')
        : join(__dirname, '../../resources/locust-permission-bridge.mjs'),
      // Electron's own binary, run as node (ELECTRON_RUN_AS_NODE is set in
      // the server's env): guaranteed present, whatever is on PATH.
      node: process.execPath,
      emitApproval: raiseApproval,
      withdrawApproval: (approvalId) => withdrawCard(approvalId)
    })
    permissionHostForShutdown = permissionHost
    const browserHost = createBrowserHost({
      bridgePath: app.isPackaged
        ? join(process.resourcesPath, 'locust-browser-bridge.mjs')
        : join(__dirname, '../../resources/locust-browser-bridge.mjs'),
      node: process.execPath,
      getWorkspaceRoot: () => (workspaceChosen ? workspacePath : undefined),
      policyRefusal: () => policyBrowserRefusal(organizationPolicy),
      emitState: (state) => {
        if (approvalWindow !== undefined && !approvalWindow.isDestroyed() && !approvalWindow.webContents.isDestroyed()) {
          approvalWindow.webContents.send(BROWSER_EVENT_CHANNEL, state)
        }
      },
      // A teammate's own sign-ins (0.839): only while its record says it has them.
      profileOf: async (teammateId) => {
        const mate = (await teammates.list().catch(() => [])).find((entry) => entry.teammateId === teammateId)
        return mate?.ownBrowser === true ? { id: mate.teammateId, name: mate.name, hue: mate.hue } : undefined
      },
      emitPointHotkey: () => {
        if (approvalWindow !== undefined && !approvalWindow.isDestroyed() && !approvalWindow.webContents.isDestroyed()) {
          approvalWindow.webContents.send(BROWSER_POINT_HOTKEY_CHANNEL)
        }
      }
    })
    browserForShutdown = browserHost
    // No teammate ends your browser (0.717): the hook every Claude Code run is given. Beside app.asar, as the bridge is.
    const commandGuardWritten = writeCommandGuard(app.getPath('userData'), {
      node: process.execPath,
      guardPath: app.isPackaged ? join(process.resourcesPath, 'locust-command-guard.mjs') : join(__dirname, '../../resources/locust-command-guard.mjs'),
      // 0.718: whose children the runs' agents are, so the guard can tell what a run started.
      parent: process.pid
    })
    // The same guard for Copilot (0.721): a plugin folder beside it, given to every Copilot run.
    const copilotGuardWritten = writeCopilotCommandGuard(app.getPath('userData'), {
      node: process.execPath,
      guardPath: app.isPackaged ? join(process.resourcesPath, 'locust-command-guard.mjs') : join(__dirname, '../../resources/locust-command-guard.mjs'),
      parent: process.pid,
      // A drive's line per question (the 0.720 seam). Copilot's env is the runner's allowlist, so the hook names it.
      ...(process.env.LOCUST_GUARD_LOG === undefined ? {} : { log: process.env.LOCUST_GUARD_LOG })
    })
    // And for OpenCode (0.722): a plugin file every run and server is told to load.
    const openCodeGuardWritten = writeOpenCodeCommandGuard(app.getPath('userData'), {
      node: process.execPath,
      guardPath: app.isPackaged ? join(process.resourcesPath, 'locust-command-guard.mjs') : join(__dirname, '../../resources/locust-command-guard.mjs'),
      parent: process.pid,
      ...(process.env.LOCUST_GUARD_LOG === undefined ? {} : { log: process.env.LOCUST_GUARD_LOG })
    })
    const openCode2GuardWritten = writeOpenCode2CommandGuard(app.getPath('userData'), {
      node: process.execPath,
      guardPath: app.isPackaged ? join(process.resourcesPath, 'locust-command-guard.mjs') : join(__dirname, '../../resources/locust-command-guard.mjs'),
      parent: process.pid,
      ...(process.env.LOCUST_GUARD_LOG === undefined ? {} : { log: process.env.LOCUST_GUARD_LOG })
    })
    // OpenCode's first-start setup names the same guard a run does (0.780).
    void openCodeGuardWritten.then(markOpenCodeGuard)
    // Ask and Plan on OpenCode have no shell: git reaches them through Locust's git tool (0.818), beside app.asar.
    const openCodeGitReadsWritten = writeOpenCodeGitReads(app.getPath('userData'), {
      node: process.execPath,
      scriptPath: app.isPackaged ? join(process.resourcesPath, 'locust-git-reads.mjs') : join(__dirname, '../../resources/locust-git-reads.mjs')
    })

    /*
     * What answers a run that stops to ask. Approve-each used to have a whole
     * second mission service for this; since every Codex mode runs on
     * app-server through the one loop, the channel is all that mode still
     * needs of its own.
     */
    const approvals = createApprovalChannel({
      emitApproval: raiseApproval,
      emitUpdate: (update) => {
        const target = approvalWindow
        if (target && !target.isDestroyed() && !target.webContents.isDestroyed()) {
          target.webContents.send(CODEX_MISSION_UPDATE_CHANNEL, update)
        }
      },
      /*
       * A denial the person explained, on Codex (0.374): its reply has no room
       * for the reason, so the run is shown it as its next input. A run that
       * can no longer take one is said so -- the reason must not vanish.
       */
      onDeniedSaying: ({ runId, missionId, reason }) => {
        void codexMissions
          .steer(runId, `I declined that. ${reason}`)
          .catch(() => false)
          .then((took) => {
            if (took) return
            const target = approvalWindow
            if (target && !target.isDestroyed() && !target.webContents.isDestroyed()) {
              target.webContents.send(CODEX_MISSION_UPDATE_CHANNEL, {
                kind: 'relay-notice',
                runId,
                missionId,
                message: 'Your reason was not passed on: the run could not take a message just then. Say it in your next message.'
              })
            }
          })
      }
    })

    // A2.14: a run's end, in order and each on its own -- its memory first,
    // so a teammate started because of this run is briefed with it.
    // Scout, a teammate that keeps watch (0.756): made further down, heard here as every run ends.
    let scout: Scout | undefined
    // Scout's fixes (0.828, scout-fix.ts): each fix's turn is waited on as runs end.
    let scoutFixes: ScoutFixes | undefined
    const runEnd = createRunEnd({
      memory: () => memoryReader,
      relay: () => relay,
      after: [
        { label: 'routine', step: () => routineRunner },
        { label: 'room tasks', step: () => roomTasks },
        // Before attention: a turn carried on is not stopped and waiting, so it raises no "limit reached" (0.774).
        { label: 'carry on', step: () => carryOn },
        {
          label: 'attention',
          step: () => {
            const reader = attentionReader
            if (reader === undefined) return undefined
            return { onRunEnded: (mission) => (carryOn?.carried(mission.missionId) === true ? Promise.resolve() : reader.onRunEnded(mission)) }
          }
        },
        { label: 'unwritten answers', step: () => unwrittenAnswers },
        { label: 'scout', step: () => scout },
        { label: 'scout fixes', step: () => scoutFixes },
        // A turn started in a chat app is answered there (0.831, 0.832), after carry-on has said whether it carries on.
        { label: 'chat apps', step: () => (chatLinks.length === 0 ? undefined : { onRunEnded: async (mission) => { for (const link of chatLinks) await link.onRunEnded(mission).catch(() => undefined) } }) }
      ],
      note
    })
    // A3.3: read fresh at every check, so a command set in Settings applies
    // to the next turn. The folder's own id: the command is per project.
    // The conversation's own folder's command (0.813, Sol's review): a conversation keeps its folder when the window
    // switches (0.458), and its check is that folder's, never the folder now on screen.
    const editCheck = createEditCheck({
      commandFor: async (workspaceId) => {
        const folder = workspaceId ?? (workspaceChosen ? workspaceIdFor(workspacePath) : undefined)
        return folder === undefined ? undefined : (await teammates.readSettings()).checkCommands?.[folder]
      }
    })
    /*
     * A teammate's monthly limit (shared/spend.ts), read fresh at every start
     * so a limit changed in the dialog applies to the very next run. Only a
     * teammate WITH a limit costs a pass over the ledger, and that pass goes
     * through the history's own cache.
     */
    const spendRefusal = createSpendRefusal({ policy: organizationPolicy, ledger: missionLedger, teammates: () => teammates })
    const policyStartRefusal = async (turn: PolicyTurn, teammateId?: string): Promise<string | undefined> =>
      policyTurnRefusal(organizationPolicy, turn) ?? await spendRefusal(teammateId)
    /*
     * 0.391: what the person did in a runtime's own terminal comes back into
     * the conversation (terminal-catch-up.ts). Read lazily: the services and
     * the roster it asks are made further down, and it is only ever called
     * after they exist.
     */
    const terminalImportsFile = join(app.getPath('userData'), 'terminal-imports.json')
    const terminalImports = createTerminalImports(terminalImportsFile)
    const sessionPlaces = {
      claudeHome: process.env.CLAUDE_CONFIG_DIR ?? join(homedir(), '.claude'),
      codexHome: process.env.CODEX_HOME ?? join(homedir(), '.codex')
    }
    const catchUpFacts = {
      ledger: missionLedger,
      newestTurnOf: (missionId) => newestTurnOf(missionLedger, missionId),
      liveMissionIds: () => [...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()],
      sessionOf: runtimeThreadIdOf,
      transcriptOf: createTranscriptReader({
        claudeHome: process.env.CLAUDE_CONFIG_DIR ?? join(homedir(), '.claude'),
        codexHome: process.env.CODEX_HOME ?? join(homedir(), '.codex')
      }),
      imports: terminalImports,
      ownerOf: async (id: string) => (await teammates.missionOwners())[id],
      assign: (teammateId: string, id: string) => teammates.assignMission(teammateId, id)
    } satisfies CatchUpFacts
    const catchUpMachine = createTerminalCatchUp(catchUpFacts)
    const catchUp = async (missionId: string) => {
      const mission = await missionLedger.getMission(missionId)
      const account = accountFromRoute(mission?.metadata.resolvedRouteId ?? '')
      if (account === undefined) return catchUpMachine(missionId)
      const own = (await agentAccounts.state()).accounts.find(a => a.id === account && a.runtime === mission?.metadata.runtime)
      if (own?.home === undefined) throw new Error('This conversation used a removed account. Its record was not read from another sign-in.')
      const places = { ...sessionPlaces, ...(own.runtime === 'claude' ? { claudeHome: own.home } : { codexHome: own.home }) }
      return catchUpTerminal(missionId, { ...catchUpFacts, transcriptOf: createTranscriptReader(places) })
    }
    // Each runtime's own slash commands, as its CLI last listed them (0.426).
    const runtimeCommands = createRuntimeCommands({ file: join(app.getPath('userData'), 'runtime-commands.json') })
    // Cursor saves a run's --model as the person's own default; put back after (0.431).
    const cursorDefaultModel = createCursorDefaultModel({
      file: join(homedir(), '.cursor', 'cli-config.json'),
      keptFile: join(app.getPath('userData'), 'cursor-default-model.json'),
      note: (what) => note('cursor-default-model', what)
    })
    void cursorDefaultModel.recover().catch(() => undefined)
    /*
     * YOUR OWN AGENTS (0.777, shared/acp-agents.ts): any program speaking the Agent Client Protocol, by the command
     * line the person gave it, found by the same locator as the agents Locust knows.
     */
    const acpAgents = createAcpAgents({
      directory: app.getPath('userData'),
      locate: (command) => executableLocator.find(command),
      spawn: (executablePath, args, env, cwd) => spawnAppServer(executablePath, args, env, cwd),
      scratch: acpScratch(join(app.getPath('temp'), 'locust-acp-tests'))
    })
    ipcMain.handle(ACP_AGENTS_LIST_CHANNEL, (event) => (fromOwnWindow(event) ? acpAgents.list() : { ok: false, message: 'Only Locust’s own window can read these.' }))
    ipcMain.handle(ACP_AGENTS_TEST_CHANNEL, (event, commandLine: unknown) =>
      fromOwnWindow(event) && typeof commandLine === 'string' ? acpAgents.test(commandLine) : { ok: false, message: 'That is not a command line.' })
    ipcMain.handle(ACP_AGENTS_SAVE_CHANNEL, (event, name: unknown, commandLine: unknown) =>
      fromOwnWindow(event) ? acpAgents.save(name, commandLine) : { ok: false, message: 'Only Locust’s own window can add an agent.' })
    ipcMain.handle(ACP_AGENTS_REMOVE_CHANNEL, (event, id: unknown) =>
      fromOwnWindow(event) ? acpAgents.remove(id) : { ok: false, message: 'Only Locust’s own window can remove an agent.' })
    const agentAccounts = createAgentAccounts({
      directory: app.getPath('userData'),
      refusal: () => policyFeatureRefusal(organizationPolicy, 'accounts'),
      inUse: async (_runtime, account) => codexMissions.accountInUse(account) || (await backgroundRuns.list()).some(run => run.account === account && ['working', 'blocked', 'unknown'].includes(run.state)),
      changed: () => { accountCatalogs.clear(); sendToWindow({ kind: 'models-changed' }) },
      login: async (runtime, home, env, done) => {
        const found = await executableLocator.find(runtime)
        if (found === undefined) throw new Error('The agent is not installed.')
        return startAccountLogin(runtime, { ...found, env: { ...found.env, ...env } }, home, done)
      }
    })
    app.on('before-quit', () => agentAccounts.dispose())
    const discoverForAccount = async (runtime?: string, account?: string): Promise<readonly RuntimeDiscovery[]> => {
      if (account === undefined) return discoverForStart(runtime)
      if (runtime === undefined || !supportsAccounts(runtime)) throw new Error('This agent does not support another account.')
      const launch = await executableLocator.find(runtime)
      if (launch === undefined) return []
      const own = await agentAccounts.launch(runtime, account, launch)
      return discoverInstalledRuntimes({ runner: probeRunner, locator: { find: async () => own }, only: new Set([runtime]), recall: runtimeFacts })
    }
    ipcMain.handle(AGENT_ACCOUNTS_CHANNEL, async (event, action: AccountAction) => {
      if (!fromOwnWindow(event)) return { ok: false, accounts: [], message: 'Only Locust\'s own window can manage accounts.' }
      try {
        if (action?.kind === 'preview' && !app.isPackaged && process.env.LOCUST_ACCOUNT_PREVIEW === '1') {
          const a = (await agentAccounts.list()).find(a => a.id === action.id)
          if (a === undefined) throw new Error('Choose an added account to preview.')
          const found = await executableLocator.find(a.runtime)
          if (found === undefined) throw new Error('The agent is not installed.')
          const own = await agentAccounts.launch(a.runtime, a.id, found)
          const variable = a.runtime === 'claude' ? 'CLAUDE_CONFIG_DIR' : 'CODEX_HOME'
          return { ...await agentAccounts.state(), preview: { runtime: a.runtime, variable, home: own.env![variable]! } }
        }
        if (action?.kind === 'add') await agentAccounts.add(action.runtime, action.name)
        else if (action?.kind === 'rename') await agentAccounts.rename(action.id, action.name)
        else if (action?.kind === 'remove') await agentAccounts.remove(action.id)
        else if (action?.kind === 'login') await agentAccounts.login(action.id)
        else if (action?.kind === 'cancel') await agentAccounts.cancel(action.id)
        else if (action?.kind !== 'list') throw new Error('Choose an account action in Settings.')
        return await agentAccounts.state()
      } catch (cause) {
        return { ok: false, accounts: await agentAccounts.list().catch(() => []), message: cause instanceof Error ? cause.message : 'The account could not be changed. Your other accounts are unchanged.' }
      }
    })
    const references = createReferenceStore({
      root: app.getPath('userData'),
      note: (message) => note('reference-cleanup-failed', message),
      parent: async (id) => (await missionLedger.getMission(id))?.metadata.continuesFrom?.missionId,
      pickFolder: async () => {
        if (!app.isPackaged && process.env.LOCUST_REFERENCE_PATH) return process.env.LOCUST_REFERENCE_PATH
        const owner = BrowserWindow.getFocusedWindow()
        if (owner === null) return undefined
        const picked = await dialog.showOpenDialog(owner, { title: 'Add a reference folder', buttonLabel: 'Add reference', properties: ['openDirectory'] })
        return picked.canceled ? undefined : picked.filePaths[0]
      }
    })
    void references.sweep().catch((error: unknown) => note('reference-cleanup-failed', error instanceof Error ? error.message : 'The reference cache was left alone.'))
    ipcMain.handle(REFERENCES_CHANNEL, async (event, request: ReferenceRequest) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'Only Locust\'s own window can choose references. Nothing was changed.' }
      try {
        if (request?.action === 'folder') {
          const item = await references.folder()
          return { ok: true, references: item === undefined ? [] : [item] }
        }
        if (request?.action === 'github' && typeof request.repository === 'string' && request.repository.length <= 240) return { ok: true, references: [await references.github(request.repository)] }
        if (request?.action === 'discard' && typeof request.id === 'string' && /^[a-f0-9-]{36}$/.test(request.id)) {
          await references.discard(request.id)
          return { ok: true, references: [] }
        }
        if ((request?.action === 'list' || request?.action === 'set') && (request.missionId === undefined || (typeof request.missionId === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(request.missionId)))) {
          if (request.missionId !== undefined && await missionLedger.getMission(request.missionId) === undefined) throw new Error('That conversation is not available. Its references were not changed.')
          if (request.action === 'set') {
            if (request.missionId === undefined) throw new Error('Open a conversation first. Nothing was changed.')
            await references.set(request.missionId, request.ids)
          }
          return { ok: true, references: await references.list(request.missionId), ...(request.missionId === undefined ? {} : { cwd: await references.cwd(request.missionId) }) }
        }
        return { ok: false, message: 'Choose a folder or GitHub repository using Add a reference. Nothing was changed.' }
      } catch (error) {
        return { ok: false, message: error instanceof Error ? error.message : 'The reference could not be added. Your conversation is unchanged.' }
      }
    })
    let claudeContextReader: ClaudeContextReader | undefined
    const codexMissions = createCodexMissionService({
      references,
      organizationPolicy,
      workspacePath,
      claudeContext: {
        beforeStart: async () => { await claudeContextReader?.beforeStart() },
        remember: async (id, launch) => { await claudeContextReader?.remember(id, launch) }
      },
      // 0.458: the window's folder now, and a conversation's own folder by id.
      currentFolder: () => workspacePath,
      folderOf: workableFolderOf,
      peerIn: (peer, folder) => peerContextFor(peer.self.teammateId, folder),
      catchUpTerminal: catchUp,
      // A2.9: the overlap note -- who else changed the files a teammate did, lately.
      recentEdits: createRecentEdits(),
      checkpoints,
      turnRecords,
      // A scripted launch spends nothing unless told to (free-routes.ts).
      freeRoutesOnly: freeRoutesOnly(process.argv, process.env),
      spendRefusal,
      teammateKeys: (teammateId) => teammateKeys.runKeys(teammateId),
      ownProvider: (model) => ownModels.providerFor(model),
      permissionHost,
      browserHost,
      commandGuard: () => commandGuardWritten,
      copilotCommandGuard: () => copilotGuardWritten,
      copilotMcpServers: () => copilotMcpServerNames(),
      openCodeCommandGuard: () => openCodeGuardWritten,
      openCode2CommandGuard: () => openCode2GuardWritten,
      openCodeGitReads: () => openCodeGitReadsWritten,
      setsItselfUp: (runtime) => runtime === 'opencode' && discoveryCache?.value.find((entry) => entry.id === 'opencode')?.version?.major === 1 && openCodeFirstStart.willSetUp(),
      // The same guard for Codex (0.720), only when it is the one hook Codex would load.
      codexCommandGuard: async (folder, codexHome) => {
        if ((await otherCodexHooks({ codexHome: codexHome ?? process.env.CODEX_HOME ?? join(homedir(), '.codex'), folder })).length > 0) return undefined
        return codexCommandGuardConfig({
          node: process.execPath,
          guardPath: app.isPackaged ? join(process.resourcesPath, 'locust-command-guard.mjs') : join(__dirname, '../../resources/locust-command-guard.mjs'),
          parent: process.pid
        })
      },
      approvals,
      // A ledger write that fails mid-run names its reason in locust-errors.log.
      note,
      // Every Codex mode rides `codex app-server`, because `codex exec --json`
      // never streams an agent message -- measured 2026-09-10, the whole reply
      // arrives as one `item.completed`. Called lazily for the same reason
      // `liveElsewhere` is: the spawner is defined further down this same
      // setup, and nothing starts a mission until all of it has run.
      appServerSpawn: (executablePath, args, env) => spawnAppServer(executablePath, args, env),
      // A6.7: OpenCode's own server, for Approve-each; the same launcher (tree
      // kill on Windows), started IN its folder (0.378) -- a session made
      // without naming one is the server's own folder's.
      opencodeServeSpawn: (executablePath, args, env, cwd) => spawnAppServer(executablePath, args, env, cwd),
      // 0.761: a turn sent with Cloud, on the repository's GitHub Codespace (cloud-turns.ts). Lazily, as above.
      cloud: {
        of: (missionId) => cloudTurns.of(missionId),
        prepare: (input) => cloudTurns.prepare(input),
        settled: (missionId) => cloudTurns.settled(missionId),
        carry: (missionId, folder, files) => cloudTurns.carry(missionId, folder, files)
      },
      // 0.377: an Agent Client Protocol agent (Copilot, for Approve-each), started IN its folder.
      acpSpawn: (executablePath, args, env, cwd) => spawnAppServer(executablePath, args, env, cwd),
      // W12: what it said it can do, said in Settings; the next sweep reads it.
      onAcpCapabilities: (runtime, capabilities) => acpCapabilities.record(runtime, capabilities),
      // 0.777: the agents the person added, each by its own command line.
      acpAgents: { resolve: (agentId) => acpAgents.resolve(agentId) },
      // A3.3: the person's check for THIS folder, after a turn that changed files.
      afterEdits: (cwd, missionId, onStart, workspaceId) => editCheck.after(cwd, missionId, onStart, workspaceId),
      onEditCheck: (turn, result) => {
        void checkFix?.onChecked(turn, result).catch(() => undefined)
      },
      // 0.439: a commit per turn on the teammate's own branch (turn-checkpoint.ts).
      checkpointTurn: ({ repositoryRoot, teammateId, message }) => createWorktreeManager({ workspacePath: repositoryRoot, note: detail => note('worktree-add', detail) }).checkpoint(teammateId, message),
      /*
       * Asked at the start of every run, never captured: a connector signed
       * into after launch reaches the next mission without a restart.
       *
       * The refresh is kicked and NOT awaited. `claude mcp list` health-checks
       * every server, so awaiting it would put seconds between pressing send
       * and anything happening. The reading it takes lands for the next run;
       * this one uses whatever is already held, which after the warm below is
       * almost always the current list.
       */
      connectors: () => {
        void connectorReader.refresh().catch(() => undefined)
        return connectorReader.names()
      },
      /*
       * The cap is ONE pool across all three transports.
       *
       * Each service counted only its own live runs, so four exec runs, four
       * approve-each runs and four Antigravity runs could be live together --
       * twelve -- while the refusal still read \"up to 4\" and every read in
       * this file already summed all three (teammateBusy, the sidebar count,
       * the busy list). Reported as one number, enforced as three.
       *
       * Read lazily rather than captured, because these three are constructed
       * in sequence and each needs the other two.
       */
      liveElsewhere: () => antigravityMissions.liveMissionIds().length,
      // Asked at the moment a run starts, never cached: switching Auto off in
      // Settings has to reach the next run, including one a relay or a saved
      // routine is about to start.
      autoModeAllowed: async () => (await teammates.readSettings()).autoMode === true,
      // Same shape, same reason: read when the run starts, so flipping the
      // switch reaches the next mission without a restart.
      askConnectors: async () => (await teammates.readSettings()).askConnectors === true,
      keepATodoList: async () => (await teammates.readSettings()).keepATodoList === true,
      // The folder's skills always; the person's own only when Settings says so, read at run start; the ones kept
      // from GitHub always, Auto included.
      claudeSkills: async (workspace, run) =>
        prepareClaudeSkills({
          scratchRoot: claudeSkillCopies,
          workspacePath: workspace,
          ownSkills: (await teammates.readSettings()).claudeOwnSkills === true,
          libraryFolder: skillLibrary.skillsFolder,
          auto: run.auto,
          // The run's teammate's own skills, and a tune column's one skill for this run alone (0.811).
          ...(run.teammateId === undefined ? {} : {
            teammateFolder: teammateSkills.folderOf(run.teammateId),
            teammateSkills: (await teammateSkills.list(run.teammateId).catch(() => [])).map((skill) => skill.name)
          }),
          ...(run.skill === undefined ? {} : { candidate: run.skill })
        }),
      // The same for Codex, OpenCode, Copilot and Antigravity (0.813): the teammate's own, a tune column's one, and the ones kept from GitHub.
      runSkills: async (run) =>
        prepareRunSkills({
          scratchRoot: runSkillCopies,
          libraryFolder: skillLibrary.skillsFolder,
          ...(run.teammateId === undefined ? {} : {
            teammateFolder: teammateSkills.folderOf(run.teammateId),
            teammateSkills: (await teammateSkills.list(run.teammateId).catch(() => [])).map((skill) => skill.name)
          }),
          ...(run.skill === undefined ? {} : { candidate: run.skill })
        }),
      readyConnectors: cursorReadyConnectors,
      // Each runtime's own slash commands (0.426, runtime-commands.ts).
      cursorDefaultModel,
      onRuntimeCommands: (runtime, commands) => {
        void runtimeCommands.set(runtime, commands).then((changed) => {
          if (changed) sendToWindow({ kind: 'runtime-commands-changed' })
        }).catch((error: unknown) => note('runtime-commands', `could not keep ${runtime}'s commands: ${error instanceof Error ? error.message : String(error)}`))
      },
      allowConnectors: async (workspace) => cursorConnectors.allow(workspace, await cursorConfiguredConnectorNames()),
      connectorsHeldIn: (workspace) => cursorConnectors.holds(workspace),
      discover: discoverForAccount,
      runner: createNodeRuntimeProcessRunner(),
      ledger: missionLedger,
      workroom,
      memory: memoryBriefing,
      // A2.5: a resumed session is told only what changed in its brief.
      briefSessions: createBriefSessions({ rootDirectory: app.getPath('userData') }),
      readablePdf: pdfReadings.prepare,
      onShared: (mission, posted) => runEnd.onShared(mission, posted),
      onRunEnded: (mission) => runEnd.onRunEnded(mission)
    })
    // The approval transport. It only runs for the mode that asked for it, so
    // an experimental protocol failing cannot take the ordinary paths with it.
    let approvalWindow: BrowserWindow | undefined
    const attention = createAttention({
      focused: () => {
        const target = approvalWindow
        return target !== undefined && !target.isDestroyed() && target.isFocused() && !target.isMinimized()
      },
      // Not from a drive: a toast from any copy points the Start-menu
      // shortcut at that copy (stale-shortcut.ts). The drive check comes
      // FIRST. MEASURED 2026-09-24: `Notification.isSupported()` alone, with
      // no toast shown, rewrites the Start-menu shortcut to the calling copy
      // -- so with the order the other way round every drive that raised an
      // approval re-pointed Colin's Locust.lnk (and the taskbar pin that
      // resolves through it) at a worktree's release folder.
      supported: () => mayShowToasts(process.argv) && Notification.isSupported(),
      notify: ({ title, body, onClick }) => {
        const toast = new Notification({ title, body })
        toast.on('click', onClick)
        toast.show()
      },
      openMission: (missionId) => {
        const target = approvalWindow
        if (target && !target.isDestroyed() && !target.webContents.isDestroyed()) {
          target.webContents.send(ATTENTION_OPEN_MISSION_CHANNEL, missionId)
        }
      },
      focusWindow: () => {
        const target = approvalWindow
        if (target === undefined || target.isDestroyed()) return
        if (target.isMinimized()) target.restore()
        target.show()
        target.focus()
      }
    })
    /*
     * Your phone's alerts (0.827, phone-alerts.ts): the moments the desk hears, by the phone's own rules. Sent through
     * phone-anywhere once that exists; Scout's quiet hours are read as Scout last said them.
     */
    let phoneAlertSink: ((alert: PhoneAlert) => void) | undefined
    let scoutQuiet: ScoutQuietHours | undefined
    let phoneNightApprovals = false
    const phoneAlerts = createPhoneAlerts({
      send: (alert, parts) => {
        phoneAlertSink?.(alert)
        // The chat apps that are on (0.837): each card still waiting, with its buttons; the rest as one message.
        const waiting = raisedApprovals.waiting()
        const cards = parts.flatMap((part) => {
          const entry = part.approvalId === undefined ? undefined : waiting.find((one) => one.request.approvalId === part.approvalId)
          return entry === undefined ? [] : [{ request: entry.request, name: part.who }]
        })
        // A card answered since it was raised is not told at all.
        const lines = parts.filter((part) => part.approvalId === undefined).map((part) => ({ title: part.title, body: part.body }))
        if (cards.length > 0 || lines.length > 0) for (const link of chatLinks) link.alert({ cards, lines })
      },
      away: () => {
        const target = approvalWindow
        return target === undefined || target.isDestroyed() || !target.isFocused() || target.isMinimized()
      },
      quiet: (at) => inQuietHours(scoutQuiet, at),
      approvalsAtNight: () => phoneNightApprovals,
      cardOpen: async (cardId) => ((await scout?.state())?.cards ?? []).some((card) => card.cardId === cardId)
    })
    app.once('will-quit', () => phoneAlerts.dispose())
    // Antigravity, experimental: driven through the running app, watched
    // through its transcript. Its own service, so its heuristics cannot leak
    // into the transports that read a process.
    const antigravityMissions = createAntigravityMissionService({
      organizationPolicy,
      // Read live (0.458): Antigravity works in whichever folder the window is in.
      get workspacePath() {
        return workspacePath
      },
      freeRoutesOnly: freeRoutesOnly(process.argv, process.env),
      spendRefusal,
      ledger: missionLedger,
      // One pool -- see the note on codexMissions above.
      liveElsewhere: () => codexMissions.liveMissionIds().length,
      workroom,
      memory: memoryBriefing,
      probe: () => antigravityProbe.probe(),
      emitEvent: (runId, missionId, event) => {
        const target = approvalWindow
        if (target && !target.isDestroyed() && !target.webContents.isDestroyed()) {
          target.webContents.send(CODEX_MISSION_UPDATE_CHANNEL, { kind: 'event', runId, missionId, event })
        }
      },
      emitUpdate: (update) => sendToWindow(update),
      // A silence that looks like a question, said in the run's own thread.
      notify: ({ runId, missionId, message }) => sendToWindow({ kind: 'relay-notice', runId, missionId, message }),
      // Its `ask_question`, as the same card Codex's questions use -- and
      // taken down again when it was answered in Antigravity's own window.
      emitApproval: raiseApproval,
      withdrawApproval: (approvalId) => withdrawCard(approvalId),
      onShared: (mission, posted) => runEnd.onShared(mission, posted),
      onRunEnded: (mission) => runEnd.onRunEnded(mission)
    })
    // 0.379: the computer stays awake while any teammate works, in either
    // transport, and not a moment longer (keep-awake.ts). The system, not
    // the screen: the display may still sleep.
    const keepAwake = createKeepAwake({
      start: () => powerSaveBlocker.start('prevent-app-suspension'),
      stop: (id) => powerSaveBlocker.stop(id)
    })
    // 0.838: plugged in, and asked to, it also stays awake with nothing running, so a routine or Scout's look comes on time.
    let stayAwake = readStayAwake(join(app.getPath('userData'), AWAKE_FILE))
    const pluggedIn = (): boolean => {
      try {
        return !powerMonitor.isOnBatteryPower()
      } catch {
        return false
      }
    }
    const awakeNow = (): void => {
      keepAwake.update(codexMissions.liveMissionIds().length + antigravityMissions.liveMissionIds().length, stayAwake && pluggedIn())
    }
    const awakeBeat = setInterval(awakeNow, KEEP_AWAKE_BEAT_MS)
    ipcMain.handle(STAY_AWAKE_GET_CHANNEL, (event) => {
      if (!fromOwnWindow(event)) return { stayAwake: false }
      return { stayAwake }
    })
    ipcMain.handle(STAY_AWAKE_SET_CHANNEL, (event, next: unknown) => {
      if (!fromOwnWindow(event) || typeof next !== 'boolean') return { stayAwake }
      try {
        writeStayAwake(join(app.getPath('userData'), AWAKE_FILE), next)
      } catch {
        // Kept for this run; the next start reads it as off.
      }
      stayAwake = next
      awakeNow()
      return { stayAwake }
    })
    awakeBeat.unref()
    app.once('will-quit', () => {
      clearInterval(awakeBeat)
      keepAwake.dispose()
    })
    const sendToWindow = (update: CodexMissionUpdate): void => {
      const target = approvalWindow
      if (target && !target.isDestroyed() && !target.webContents.isDestroyed()) {
        target.webContents.send(CODEX_MISSION_UPDATE_CHANNEL, update)
      }
    }

    /*
     * Discovery, pushed to the window as it happens.
     *
     * Everything already recorded goes first, because the window is created
     * while the first sweep is running and a screen that joined halfway
     * would be missing the beginning of its own log.
     */
    /*
     * To EVERY window, not just the app's.
     *
     * This sent only to `approvalWindow`, and the loading window is a
     * second window -- so the splash pulled the backlog once and then never
     * heard another word, which meant it never saw discovery finish and
     * never closed. The app sat behind it, built and invisible.
     *
     * Discovery is a fact about the machine, not about one window, so every
     * window that is open gets it.
     */
    const sendDiscovery = (event: DiscoveryEvent): void => {
      for (const target of BrowserWindow.getAllWindows()) {
        if (!target.isDestroyed() && !target.webContents.isDestroyed()) {
          target.webContents.send(RUNTIME_DISCOVERY_EVENT_CHANNEL, event)
        }
      }
    }
    discoveryLog.subscribe(sendDiscovery)
    /*
     * REPLAY WHEN THERE IS A WINDOW, not when this line runs.
     *
     * `approvalWindow` is assigned at the end of startup, so everything
     * emitted before then -- `context` among it, which is emitted once per
     * launch -- reached a `sendDiscovery` with nowhere to send. Measured on
     * the first drive: the screen opened at "scanning PATH" with no
     * preamble above it, because the three lines above it had already been
     * spoken to an empty room.
     */
    const replayDiscoveryToWindow = (): void => {
      for (const event of discoveryLog.replay()) sendDiscovery(event)
    }
    // And the renderer can ask, which is what actually works: a replay sent
    // at window creation lands before the page has mounted a listener.
    ipcMain.handle(RUNTIME_DISCOVERY_LOG_CHANNEL, () => discoveryLog.replay())
    /*
     * The loading window is finished: show the app and close the splash, in
     * that order. Closing first would leave an empty desktop for a frame.
     */
    ipcMain.on(SPLASH_DONE_CHANNEL, () => {
      showAppWindow()
      const splash = splashWindow
      splashWindow = undefined
      if (splash !== undefined && !splash.isDestroyed()) splash.close()
    })

    /*
     * What this launch is, said once and only while somebody is waiting.
     *
     * Every value here is read, never composed for the screen: the version
     * the app reports, the platform it is on, the folder it opened, the
     * branch that folder is on, and whether the ledger directory can
     * actually be written to. `ledgerOk` is a real write test rather than a
     * guess, because "your ledger is fine" is exactly the kind of
     * reassurance that must not be invented.
     */
    void (async () => {
      const readGit = (args: readonly string[]): Promise<string | undefined> =>
        new Promise((resolve) => {
          execFile('git', ownGitArgs(args), { cwd: workspacePath, windowsHide: true }, (error, stdout) => {
            resolve(error === null ? stdout.trim() : undefined)
          })
        })
      const branch = await readGit(['rev-parse', '--abbrev-ref', 'HEAD'])
      const dirty = await readGit(['status', '--porcelain'])
      // A real write test, not a guess. "Your ledger is fine" is exactly the
      // kind of reassurance that must not be invented.
      let ledgerOk = false
      try {
        await mkdir(ledgerDirectory, { recursive: true })
        await writeFile(join(ledgerDirectory, '.writable'), '', 'utf8')
        ledgerOk = true
      } catch {
        ledgerOk = false
      }
      discoveryLog.emit({
        kind: 'context',
        version: app.getVersion(),
        platform: `${process.platform === 'win32' ? 'Windows' : process.platform === 'darwin' ? 'macOS' : 'Linux'} ${release()}`,
        workspace: workspaceChosen ? basename(workspacePath) || workspacePath : 'no folder',
        ...(branch === undefined || branch.length === 0 || branch === 'HEAD' ? {} : { branch }),
        ...(dirty === undefined ? {} : { clean: dirty.length === 0 }),
        ledgerPath: ledgerDirectory,
        ledgerOk
      })
    })()

    // Started and stopped in one place for the mission transport and the model
    // probe -- stopped once, without holding the app; see app-server-process.ts.
    const spawnAppServer = (executablePath: string, args: readonly string[], env?: Readonly<Record<string, string>>, cwd?: string) =>
      startAppServerProcess(executablePath, args, undefined, env, cwd)

    // Installing is its own service: one at a time, and it asks discovery
    // again after a clean exit rather than trusting npm's exit code alone.
    const runtimeInstaller = createRuntimeInstaller({
      // The npm this build carries, used only when the machine has none.
      ...(bundledNpm === undefined ? {} : { bundledNpm }),
      systemNpm: npmPresent,
      // Off Windows, a global folder that is the system's is stepped round into this user's own (0.515).
      ...(process.platform === 'win32' ? {} : { userPrefix: join(homedir(), '.npm-global') }),
      nowInstalled: async (runtime) => {
        discoveryCache = undefined
        runtimeDiscovery.invalidate()
        const found = await discoverForWork()
        return found.some((entry) => entry.id === runtime && entry.availability === 'available')
      }
    })

    /*
     * The models Claude Code's own picker lists (0.697), from the same
     * handshake that lists its commands: the names the catalog and every
     * chip use, so a new Claude model shows up the day Claude Code knows it
     * (Colin, 2026-10-07: "haiku 5.5 is appearing on cursor but not claude
     * on locust"). Held for the session; until it answers, the copied table
     * in shared/claude-models.ts stands in.
     */
    let claudeListed: readonly ClaudeListedModel[] | undefined
    const modelCatalog = createModelCatalog({
      discover: discoverForStart,
      spawn: spawnAppServer,
      claudeListed: () => claudeListed,
      // Antigravity's usage came after the list went out (0.709): read again to show it.
      onLateUsage: () => sendToWindow({ kind: 'models-changed' })
    })
    const accountCatalogs = new Map<string, ReturnType<typeof createModelCatalog>>()
    const readAccountModels = async () => {
      const accounts = await agentAccounts.list()
      const readings = await Promise.all(accounts.filter(a => a.state === 'ready').map(async a => {
        let catalog = accountCatalogs.get(a.id)
        if (catalog === undefined) {
          catalog = createModelCatalog({ discover: () => discoverForAccount(a.runtime, a.id), spawn: spawnAppServer })
          accountCatalogs.set(a.id, catalog)
        }
        return { account: a, result: await catalog.read() }
      }))
      return readings
    }
    const adoptClaudeListed = (models: readonly ClaudeListedModel[]): void => {
      if (claudeListed !== undefined && JSON.stringify(claudeListed) === JSON.stringify(models)) return
      claudeListed = models
      modelCatalog.forget()
      sendToWindow({ kind: 'models-changed' })
    }
    // Antigravity answered after the sweep went on without it: the list built without it is no longer the list.
    forgetModelCatalog = () => modelCatalog.forget()

    /*
     * KEEPING THE CODING AGENTS CURRENT (runtime-updates.ts). Colin,
     * 2026-09-23: "is there a way to make it so the models will automatically
     * update without messing up load times or interfering with the app". The
     * first look waits until the app has been up a while, never on the way
     * up; after it, an hourly tick that goes to the network only when a look
     * is due; an update only of what nothing is using -- and never from a
     * scripted launch (mayUpdateAgents). After one, the machine is asked
     * again and the models re-read, so the new ones are in the picker.
     */
    const updatesFile = join(app.getPath('userData'), 'runtime-updates.json')
    // A copy that is not the installed Locust changes nothing on the machine (0.514).
    // Installed: the Windows uninstaller beside it, or, on a Mac, an app in a folder it owns (0.516).
    const agentsMayUpdate = mayUpdateAgents(
      process.argv,
      process.env,
      !app.isPackaged || existsSync(join(dirname(process.execPath), 'Uninstall Locust.exe')) || (process.platform === 'darwin' && macSelfUpdateTarget(process.execPath) !== undefined)
        || (process.platform === 'linux' && linuxInstallOf(process.env, process.resourcesPath) !== undefined)
    )
    // And says so, rather than showing a switch that will not do what it says.
    const toldHere = <T extends object>(state: T): T => (agentsMayUpdate ? state : { ...state, heldHere: true })
    const runtimeUpdates = createRuntimeUpdates({
      discover: discoverForWork,
      npmRoot: npmGlobalRoot,
      latest: npmRelease,
      heldVersions: canaryHeldVersions,
      inUse: processesUsing,
      install: async (runtime, version) => {
        npmSeen = undefined
        const outcome = await runtimeInstaller.install({ runtime, version, onLine: () => undefined })
        return outcome.ok ? { ok: true } : { ok: false, what: outcome.what }
      },
      load: async () => savedUpdatesFrom(JSON.parse(await readFile(updatesFile, 'utf8'))),
      save: (saved) => writeFile(updatesFile, JSON.stringify(saved), 'utf8'),
      updated: () => {
        discoveryCache = undefined
        runtimeDiscovery.invalidate()
        modelCatalog.forget()
        // And the window is told, as a newly listed Claude model tells it: forgetting alone left the picker on the
        // old list until something else asked (0.738 sweep, drive-agents-kept-current: no GPT-6-Sol after 0.153 -> 0.162.1).
        sendToWindow({ kind: 'models-changed' })
      },
      changed: () => {
        void runtimeUpdates.state().then(toldHere).then((state) => {
          for (const window of BrowserWindow.getAllWindows()) {
            if (!window.isDestroyed()) window.webContents.send(RUNTIME_UPDATES_EVENT_CHANNEL, state)
          }
        })
      }
    })
    // Undo a turn (0.674). Ids in, states out; only a run Locust recorded can be undone.
    ipcMain.handle(TURN_UNDO_STATE_CHANNEL, async (event, runIds: unknown) => {
      if (!fromOwnWindow(event) || !Array.isArray(runIds)) return {}
      const states: Record<string, TurnUndoState> = {}
      for (const runId of runIds.filter((id): id is string => typeof id === 'string').slice(0, 200)) {
        states[runId] = await turnRecords.state(runId, checkpoints).catch((): TurnUndoState => ({ kind: 'none' }))
      }
      return states
    })
    ipcMain.handle(TURN_UNDO_CHANNEL, (event, runId: unknown) =>
      fromOwnWindow(event) && typeof runId === 'string'
        ? turnRecords.undo(runId, checkpoints, () => new Date()).catch((): TurnUndoState => ({ kind: 'none' }))
        : ({ kind: 'none' } as TurnUndoState)
    )
    // Commit (0.680): the open folder only -- no path crosses the bridge, and the commit is the person's press.
    const folderCommits = createFolderCommits()
    /*
     * A COMMIT MESSAGE WRITTEN FROM THE CHANGES (0.771, shared/commit-draft.ts): one short read-only run on the route
     * of the conversation's latest turn -- the teammate that made the changes -- given what a commit would take,
     * in a slot of its own, never a conversation (panel-runs.ts). Answered once it is done, with the message
     * for the box; the commit is still the person's press.
     */
    const panelRuns = createPanelRuns(join(app.getPath('userData'), 'panel-runs.json'))
    ipcMain.handle(PANEL_RUNS_CHANNEL, async (event) => (fromOwnWindow(event) ? panelRuns.list().catch(() => []) : []))
    ipcMain.handle(COMMIT_DRAFT_CHANNEL, async (event, of: unknown, asks: unknown): Promise<CommitDraftResponse> => {
      const kept = 'The message you had is still in the box.'
      const no = (why: string): CommitDraftResponse => ({ ok: false, message: `${why} ${kept}` })
      if (!fromOwnWindow(event)) return no('That request was not from this window.')
      const ofId = typeof of === 'string' ? of : undefined
      const turn = ofId === undefined ? undefined : await missionLedger.getMission(ofId).catch(() => undefined)
      if (ofId === undefined || turn === undefined) return no('That conversation is not in the record.')
      const pending = await folderCommits.pending(workspacePath).catch(() => undefined)
      if (pending === undefined) return no('There is nothing here to commit now.')
      const words = Array.isArray(asks) ? asks.filter((ask): ask is string => typeof ask === 'string').slice(-5) : []
      const effort = (turn.metadata as { readonly effort?: unknown }).effort
      const response = await codexMissions.start(
        commitDraftPrompt(pending, words),
        turn.metadata.runtime,
        'ask',
        { ...(turn.metadata.model === 'account-default' ? {} : { model: turn.metadata.model }), ...(accountFromRoute(turn.metadata.resolvedRouteId) === undefined ? {} : { account: accountFromRoute(turn.metadata.resolvedRouteId) }), ...(typeof effort === 'string' ? { effort } : {}) },
        sendToWindow,
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        { key: `commit-draft:${ofId}` }
      )
      if (!response.ok) return no(response.error.code === 'RUN_ALREADY_ACTIVE' ? 'A message is being written already.' : response.error.message)
      const { missionId, runId } = response.data
      // Kept before it is drawn anywhere, so the window never lists it among the conversations.
      await panelRuns.add(missionId).catch(() => undefined)
      sendToWindow({ kind: 'mission-started', runId, missionId, prompt: 'Write the commit message', data: response.data, startedBy: { kind: 'commit-draft' } })
      for (let waited = 0; waited < 180 && codexMissions.hasMission(missionId); waited += 1) await new Promise((done) => setTimeout(done, 1_000))
      if (codexMissions.hasMission(missionId)) {
        codexMissions.cancel(runId)
        return no('It took too long to write.')
      }
      const recorded = await missionLedger.getMission(missionId).catch(() => undefined)
      const tracker = createTranscriptTracker()
      tracker.track(recorded?.events ?? [])
      const message = draftedCommitMessage(tracker.latestFinal ?? '')
      return message === undefined ? no('It did not answer with a message.') : { ok: true, message }
    })
    /*
     * WALK ME THROUGH THIS CHANGE (0.771, shared/walkthrough.ts): one short read-only run on the route of the
     * conversation's latest turn, given the change the Changes panel shows -- all of it, or one commit -- with what was
     * asked and what the teammate said when it finished, never a conversation (panel-runs.ts). Answered once it is
     * done, for the panel to show; it changes nothing.
     */
    ipcMain.handle(WALKTHROUGH_CHANNEL, async (event, of: unknown, asks: unknown, sha: unknown): Promise<WalkthroughResponse> => {
      const no = (why: string): WalkthroughResponse => ({ ok: false, message: `${why} The changes are as they were.` })
      if (!fromOwnWindow(event)) return no('That request was not from this window.')
      const ofId = typeof of === 'string' ? of : undefined
      const turn = ofId === undefined ? undefined : await missionLedger.getMission(ofId).catch(() => undefined)
      if (ofId === undefined || turn === undefined) return no('That conversation is not in the record.')
      const one = typeof sha === 'string' && /^[0-9a-f]{40,64}$/.test(sha) ? sha : undefined
      const change = await folderCommits.diff(workspacePath, one).catch(() => undefined)
      const paths = change?.kind === 'diff' ? changedPaths(change.text) : []
      if (change === undefined || change.kind !== 'diff' || paths.length === 0) return no('There is no change here to walk through.')
      const said = createTranscriptTracker()
      said.track(turn.events)
      const words = Array.isArray(asks) ? asks.filter((ask): ask is string => typeof ask === 'string').slice(-5) : []
      const effort = (turn.metadata as { readonly effort?: unknown }).effort
      const response = await codexMissions.start(
        walkthroughPrompt({ diff: change.text, cut: change.truncated, paths, asks: words, ...(said.latestFinal === undefined ? {} : { said: said.latestFinal }) }),
        turn.metadata.runtime,
        'ask',
        { ...(turn.metadata.model === 'account-default' ? {} : { model: turn.metadata.model }), ...(accountFromRoute(turn.metadata.resolvedRouteId) === undefined ? {} : { account: accountFromRoute(turn.metadata.resolvedRouteId) }), ...(typeof effort === 'string' ? { effort } : {}) },
        sendToWindow,
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        { key: `walkthrough:${ofId}` }
      )
      if (!response.ok) return no(response.error.code === 'RUN_ALREADY_ACTIVE' ? 'It is being walked through already.' : response.error.message)
      const { missionId, runId } = response.data
      // Kept before it is drawn anywhere, so the window never lists it among the conversations.
      await panelRuns.add(missionId).catch(() => undefined)
      sendToWindow({ kind: 'mission-started', runId, missionId, prompt: 'Walk me through this change', data: response.data, startedBy: { kind: 'walkthrough' } })
      for (let waited = 0; waited < 240 && codexMissions.hasMission(missionId); waited += 1) await new Promise((done) => setTimeout(done, 1_000))
      if (codexMissions.hasMission(missionId)) {
        codexMissions.cancel(runId)
        return no('It took too long.')
      }
      const recorded = await missionLedger.getMission(missionId).catch(() => undefined)
      const reader = createTranscriptTracker()
      reader.track(recorded?.events ?? [])
      const walkthrough = readWalkthrough(reader.latestFinal ?? '', paths)
      return walkthrough === undefined ? no('It did not answer with a walkthrough.') : { ok: true, walkthrough }
    })
    ipcMain.handle(FOLDER_CHANGES_CHANNEL, (event) =>
      fromOwnWindow(event)
        ? folderCommits.changes(workspacePath).catch((): FolderChanges => ({ kind: 'none', why: 'not-a-repository' }))
        : ({ kind: 'none', why: 'not-a-repository' } as FolderChanges)
    )
    // Whether the open folder is a git repository, for the Changes button (2026-10-16); undefined when git cannot say.
    ipcMain.handle(FOLDER_REPOSITORY_CHANNEL, (event) =>
      fromOwnWindow(event) ? folderCommits.repository(workspacePath).catch(() => undefined) : undefined
    )
    // The folder's pull request, for the header (0.720, shared/pull-request.ts): read with the person's own gh.
    ipcMain.handle(FOLDER_PULL_REQUEST_CHANNEL, (event) =>
      fromOwnWindow(event) ? folderCommits.pullRequest(workspacePath).catch(() => undefined) : undefined
    )
    /*
     * A pull request linked in a conversation (shared/pull-request.ts). The window sends an owner, a repo and a
     * number; this runs gh and answers with the state, or nothing when gh cannot. It never prints what gh said.
     */
    const pullLinkReader = createPullLinkReader({
      gh: (args) => new Promise((resolve, reject) => {
        execFile(ghCommand(), [...args], {
          windowsHide: true, timeout: 30_000, maxBuffer: 1024 * 1024,
          env: { ...process.env, GH_PROMPT_DISABLED: '1', GH_NO_UPDATE_NOTIFIER: '1' }
        }, (error, stdout) => { if (error) reject(error); else resolve(String(stdout)) })
      })
    })
    ipcMain.handle(GITHUB_PULL_LINK_CHANNEL, (event, request: unknown) =>
      fromOwnWindow(event) ? pullLinkReader.read(request) : undefined
    )
    ipcMain.handle(GITHUB_ISSUE_LINK_CHANNEL, (event, request: unknown) =>
      fromOwnWindow(event) ? pullLinkReader.readIssue(request) : undefined
    )
    // The folder's changes in one panel (0.732): the whole change, or one of the branch's commits.
    ipcMain.handle(FOLDER_DIFF_CHANNEL, (event, sha: unknown) =>
      fromOwnWindow(event) && (sha === undefined || (typeof sha === 'string' && /^[0-9a-f]{40,64}$/.test(sha)))
        ? folderCommits.diff(workspacePath, sha as string | undefined).catch(() => ({ kind: 'none', why: 'not-a-repository' }) as const)
        : ({ kind: 'none', why: 'not-a-repository' } as const)
    )
    /*
     * Watching the folder's pull request (0.731, pull-request-watch.ts): news goes to every window, and as a
     * notification while Locust is in the background -- a click brings it forward. It never starts a run.
     */
    const pullRequestWatch = createPullRequestWatch({
      read: () => folderCommits.pullRequest(workspacePath),
      onNews: (news) => {
        for (const window of BrowserWindow.getAllWindows()) {
          if (!window.isDestroyed() && !window.webContents.isDestroyed()) window.webContents.send(PULL_REQUEST_NEWS_CHANNEL, news)
        }
        attention.runFinished({ title: `Pull request #${String(news.number)}`, body: `${news.said}.`, missionId: undefined })
      }
    })
    app.once('will-quit', () => pullRequestWatch.stop())
    ipcMain.handle(PULL_REQUEST_WATCH_CHANNEL, (event, url: unknown, on: unknown) => {
      if (!fromOwnWindow(event) || typeof url !== 'string' || !/^https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/pull\/\d+$/.test(url)) return false
      pullRequestWatch.watch(url, on === true)
      return pullRequestWatch.watching(url)
    })
    const githubIssues = createGithubIssues({
      folder: () => workspaceChosen ? workspacePath : undefined,
      originsFile: join(app.getPath('userData'), 'issue-origins.json')
    })
    const issueNumber = (value: unknown): number | undefined =>
      typeof value === 'number' && Number.isInteger(value) && value >= 1 && value < 1_000_000_000 ? value : undefined
    ipcMain.handle(FOLDER_COMMIT_CHANNEL, (event, message: unknown, then: unknown, shown: unknown, fixes: unknown) =>
      fromOwnWindow(event) && typeof message === 'string' && message.length <= 20_000 && (then === 'commit' || then === 'push' || then === 'pull-request')
        && Array.isArray(shown) && shown.length <= 100_000 && shown.every((path) => typeof path === 'string' && path.length <= 4_096)
        && (fixes === undefined || issueNumber(fixes) !== undefined)
        ? folderCommits.commit(workspacePath, message, then, shown as string[], issueNumber(fixes)).catch((error: unknown): CommitResult => ({ kind: 'refused', message: error instanceof Error ? error.message : 'The commit could not be made.' }))
        : ({ kind: 'refused', message: 'That commit request was not understood.' } as CommitResult)
    )
    const onlyKey = (payload: unknown, key: string): Record<string, unknown> | undefined => {
      if (typeof payload !== 'object' || payload === null || Array.isArray(payload)) return undefined
      const row = payload as Record<string, unknown>
      return Object.keys(row).every((name) => name === key) ? row : undefined
    }
    ipcMain.handle(GITHUB_ISSUES_READY_CHANNEL, (event, payload: unknown) =>
      fromOwnWindow(event) && payload === undefined ? githubIssues.ready() : false
    )
    ipcMain.handle(GITHUB_ISSUES_LIST_CHANNEL, (event, payload: unknown) =>
      fromOwnWindow(event) && payload === undefined ? githubIssues.list() : []
    )
    ipcMain.handle(GITHUB_ISSUE_OPEN_CHANNEL, async (event, payload: unknown) => {
      const row = onlyKey(payload, 'number')
      const number = row === undefined ? undefined : issueNumber(row.number)
      if (!fromOwnWindow(event) || number === undefined) return undefined
      const opened = await githubIssues.open(number)
      if (opened === undefined) return undefined
      return {
        brief: opened.brief,
        issue: opened.issue,
        images: opened.images.map((image) => ({ name: image.name, base64: Buffer.from(image.bytes).toString('base64') }))
      }
    })
    ipcMain.handle(GITHUB_ISSUE_REMEMBERED_CHANNEL, (event, payload: unknown) => {
      const row = onlyKey(payload, 'missionId')
      if (!fromOwnWindow(event) || row === undefined || typeof row.missionId !== 'string') return undefined
      return githubIssues.remembered(row.missionId)
    })
    ipcMain.handle(RUNTIME_UPDATES_CHANNEL, (event) =>
      fromOwnWindow(event) ? runtimeUpdates.state().then(toldHere) : { automatic: false, checkedAt: undefined, agents: [] }
    )
    ipcMain.handle(RUNTIME_UPDATES_SET_CHANNEL, (event, automatic: unknown) =>
      (fromOwnWindow(event) && typeof automatic === 'boolean' ? runtimeUpdates.setAutomatic(automatic) : runtimeUpdates.state()).then(toldHere)
    )
    // Update pressed on a runtime's row. Only an id crosses the bridge, and
    // only one of the agents kept current is ever updated for it.
    ipcMain.handle(RUNTIME_UPDATES_NOW_CHANNEL, (event, runtime: unknown) =>
      (fromOwnWindow(event) && typeof runtime === 'string' ? runtimeUpdates.updateNow(runtime) : runtimeUpdates.state()).then(toldHere)
    )
    /*
     * Undo a turn (0.674): the window's folder is kept once, a while after launch, so its first turn does not
     * wait for every file to be copied in (13 s for Locust's own checkout; 0.5 s a look once kept). Quietly:
     * a folder too large to keep says so to its turns, not here.
     */
    setTimeout(() => void checkpoints.take(workspacePath).catch(() => undefined), 15_000)
    if (agentsMayUpdate) {
      setTimeout(() => void runtimeUpdates.tick(), FIRST_LOOK_AFTER_MS)
      setInterval(() => void runtimeUpdates.tick(), 60 * 60 * 1000)
    }


    /**
     * Install a runtime, streaming npm's output to the window that asked.
     *
     * The installer runs one at a time and refuses a second; the exact
     * command it spawns is the same string the screen showed, so the app
     * cannot display one thing and run another. After a clean exit the
     * discovery cache is dropped and the machine is asked again -- which is
     * how "npm said fine but there is still nothing to run" is DETECTED
     * rather than guessed at.
     */
    ipcMain.handle(RUNTIME_INSTALL_CHANNEL, async (event, runtime: unknown) => {
      if (!fromOwnWindow(event) || typeof runtime !== 'string') {
        return { ok: false, what: 'That runtime cannot be installed from here.', next: 'Use the command shown.' } as const
      }
      const target = BrowserWindow.fromWebContents(event.sender)
      // Whatever this install does, it may have been the thing that put npm
      // on the machine -- or proved it is not there.
      npmSeen = undefined
      return runtimeInstaller.install({
        runtime,
        onLine: ({ line }) => {
          if (target !== null && !target.isDestroyed()) {
            target.webContents.send(RUNTIME_INSTALL_PROGRESS_CHANNEL, { runtime, line })
          }
        }
      })
    })

    // The runtime's own sign-in, in a window of its own. Only an id crosses
    // the bridge; the command is the one the install facts name.
    ipcMain.handle(RUNTIME_SIGN_IN_CHANNEL, async (event, runtime: unknown, again: unknown) => {
      if (!fromOwnWindow(event) || typeof runtime !== 'string') {
        return { ok: false, what: 'That runtime cannot be signed in from here.', next: 'Run the command shown in a terminal.' } as const
      }
      return openSignIn(runtime, {
        discover: discoverForWork,
        // Signing in changes nothing Locust caches except the answer itself,
        // so dropping that is enough for the next ask to be a real one.
        closed: () => {
          discoveryCache = undefined
          runtimeDiscovery.invalidate()
          // The row was left saying "Finish in the window that opened" after it closed (Sol, 0.544).
          if (!event.sender.isDestroyed()) event.sender.send(RUNTIME_SIGN_IN_CLOSED_CHANNEL, runtime)
        }
      }, again === true)
    })

    /** The window's model list, after the organization's policy; other apps' tools read the same one (0.842). */
    const readModelCatalog = async () => {
      const [read, accounts] = await Promise.all([modelCatalog.read(), policyFeatureRefusal(organizationPolicy, 'accounts') === undefined ? readAccountModels().catch(() => []) : Promise.resolve([])])
      const accountModels = accounts.flatMap(({ result }) => result.ok ? result.data.models : [])
      const accountUsage = Object.fromEntries(accounts.flatMap(({ account, result }) => !result.ok ? [] : Object.entries(result.data.usageWindows ?? {}).map(([runtime, said]) => [`${runtime}:${account.id}`, said])))
      // The person's own, first, read fresh: adding one must show at once,
      // not after the catalogue's ten-minute cache (0.357).
      const own = await ownModels.catalog().catch(() => [])
      // And the agents the person added (0.777), read fresh as their own models are.
      const added = await acpAgents.list().then((listed) => (listed.ok ? acpCatalog(listed.agents) : [])).catch(() => [])
      if (own.length === 0 && added.length === 0 && !read.ok) return read
      // The usage reading rides along with the models it was read beside (0.390).
      return { ok: true, data: { ...(read.ok ? read.data : {}), usageWindows: { ...(read.ok ? read.data.usageWindows : {}), ...accountUsage }, models: policyModels(organizationPolicy, [...new Map([...own, ...added, ...(read.ok ? read.data.models : []), ...accountModels].map(model => [`${model.runtime}:${model.id}`, model])).values()]) } } as const
    }
    ipcMain.handle(MODEL_CATALOG_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'MODELS_UNAVAILABLE', message: 'Models could not be read.' } } as const
      }
      return readModelCatalog()
    })

    ipcMain.handle(OWN_MODEL_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, error: { code: 'OWN_MODELS_UNAVAILABLE', message: 'Your own models could not be read.' } } as const
      try {
        return { ok: true, data: { models: await ownModels.list() } } as const
      } catch {
        return { ok: false, error: { code: 'OWN_MODELS_UNAVAILABLE', message: 'Your own models could not be read.' } } as const
      }
    })
    ipcMain.handle(OWN_MODEL_ADD_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, error: { code: 'OWN_MODEL_REFUSED', message: 'That model could not be added.' } } as const
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        const model = await ownModels.add({ name: input.name, baseUrl: input.baseUrl, model: input.model, key: input.key, chatOnly: input.chatOnly })
        return { ok: true, data: { model } } as const
      } catch (error) {
        // The store's own sentence when it refused; nothing about the key.
        return { ok: false, error: { code: 'OWN_MODEL_REFUSED', message: error instanceof OwnModelRefusal ? error.message : 'That model could not be added.' } } as const
      }
    })
    ipcMain.handle(OWN_MODEL_CHAT_ONLY_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, error: { code: 'OWN_MODEL_REFUSED', message: 'That model could not be changed.' } } as const
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        return { ok: true, data: { model: await ownModels.setChatOnly(input.ownId, input.chatOnly) } } as const
      } catch (error) {
        return { ok: false, error: { code: 'OWN_MODEL_REFUSED', message: error instanceof OwnModelRefusal ? error.message : 'That model could not be changed.' } } as const
      }
    })
    ipcMain.handle(OWN_MODEL_REMOVE_CHANNEL, async (event, ownId: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, error: { code: 'OWN_MODEL_REFUSED', message: 'That model could not be removed.' } } as const
      try {
        await ownModels.remove(ownId)
        return { ok: true, data: {} } as const
      } catch {
        return { ok: false, error: { code: 'OWN_MODEL_REFUSED', message: 'That model could not be removed.' } } as const
      }
    })
    ipcMain.handle(OWN_MODEL_TEST_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, error: { code: 'OWN_MODEL_REFUSED', message: 'That model could not be tested.' } } as const
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        // A kept model is tested with its kept key, which never leaves this process.
        if (typeof input.ownId === 'string') {
          const kept = (await ownModels.list()).find((model) => model.ownId === input.ownId)
          if (kept === undefined) return { ok: false, error: { code: 'OWN_MODEL_REFUSED', message: 'That model is no longer kept.' } } as const
          const key = await ownModels.keyOf(kept.ownId)
          const result = await testOwnEndpoint({ baseUrl: kept.baseUrl, model: kept.model, ...(key === undefined ? {} : { key }) })
          // A kept model found unable to take tools is set to chat only, as
          // the form sets a new one: the next run would otherwise fail on it.
          if (result.tools === false && !kept.chatOnly) await ownModels.setChatOnly(kept.ownId, true)
          if (result.tools === true && kept.chatOnly) await ownModels.setChatOnly(kept.ownId, false)
          return { ok: true, data: { reached: result.ok, said: result.said, ...(result.tools === undefined ? {} : { tools: result.tools }) } } as const
        }
        const baseUrl = ownModelAddress(input.baseUrl)
        const model = ownModelId(input.model)
        if (baseUrl === undefined || model === undefined) {
          return { ok: false, error: { code: 'OWN_MODEL_REFUSED', message: 'Fill in the address and the model first.' } } as const
        }
        const key = typeof input.key === 'string' && input.key.trim().length > 0 ? input.key.trim() : undefined
        const result = await testOwnEndpoint({ baseUrl, model, ...(key === undefined ? {} : { key }) })
        return { ok: true, data: { reached: result.ok, said: result.said, ...(result.tools === undefined ? {} : { tools: result.tools }) } } as const
      } catch {
        return { ok: false, error: { code: 'OWN_MODEL_REFUSED', message: 'That model could not be tested.' } } as const
      }
    })

    // What the person already set up inside the CLIs themselves. Read-only,
    // and gated on discovery: a leftover `.claude/agents` from an uninstall
    // must not read as a working teammate's routine.
    ipcMain.handle(RUNTIME_ARTIFACTS_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return []
      try {
        const runtimes = await discoverForWork()
        // Signed out still counts as installed: the agents they wrote are on
        // this machine whether or not the CLI can run right now.
        const installed = runtimes
          .filter(
            (runtime: RuntimeDiscovery) =>
              runtime.readiness === 'ready' || runtime.readiness === 'authentication-required'
          )
          .map((runtime: RuntimeDiscovery) => runtime.id)
        return await readRuntimeArtifacts({ installed })
      } catch {
        // A list nobody can read is an empty list, not a broken screen.
        return []
      }
    })

    ipcMain.handle(APPROVAL_RULES_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      try {
        return { ok: true, rules: await approvalRules.list() } as const
      } catch (error) {
        return { ok: false, message: error instanceof Error ? error.message : 'The saved rules could not be read. Every card asks until they can.' } as const
      }
    })
    ipcMain.handle(APPROVAL_RULES_REMOVE_CHANNEL, async (event, ruleId: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      try {
        await approvalRules.remove(ruleId)
        return { ok: true, rules: await approvalRules.list() } as const
      } catch (error) {
        return { ok: false, message: error instanceof Error ? error.message : 'That rule could not be removed. It still answers cards; try again.' } as const
      }
    })
    // Every saved rule at once (0.587, QA's Q7): the window asks, the host clears, every card asks again.
    ipcMain.handle(APPROVAL_RULES_REMOVE_ALL_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      try {
        await approvalRules.removeAll()
        return { ok: true, rules: await approvalRules.list() } as const
      } catch (error) {
        return { ok: false, message: error instanceof Error ? error.message : 'The rules could not be removed. They still answer cards; try again.' } as const
      }
    })
    // A card's "Yes, and don't ask again" / "No, and never": the rule is made HERE, from the request held.
    ipcMain.handle(APPROVAL_RULE_FROM_CARD_CHANNEL, async (event, input: unknown) => {
      if (!fromOwnWindow(event) || typeof input !== 'object' || input === null) return { ok: false, message: 'That request was rejected.' } as const
      const { approvalId, effect, reason } = input as Record<string, unknown>
      const raised = typeof approvalId === 'string' ? raisedApprovals.get(approvalId) : undefined
      if (raised === undefined || (effect !== 'allow' && effect !== 'deny')) return { ok: false, message: 'That card is no longer waiting.' } as const
      // The host's own guard (0.598): a command that reaches other programs is never a rule,
      // whatever the window asked; the card hides the offer, this is what stands when it does not.
      const refusal = ruleRefusalFor(raised.request)
      if (refusal !== undefined) return { ok: false, message: refusal } as const
      const candidate = ruleCandidateOf(ruledActionOf(raised.request), {
        ...(raised.teammateId === undefined ? {} : { teammateId: raised.teammateId }),
        ...(raised.request.cwd === null ? {} : { folder: raised.request.cwd })
      })
      if (candidate === undefined) return { ok: false, message: 'This one cannot be saved as a rule exactly, so it was not saved. Answer the card as usual.' } as const
      const decided = effect === 'allow'
        ? { approvalId: raised.request.approvalId, decision: 'approve-once' as const }
        : { approvalId: raised.request.approvalId, decision: 'deny' as const, ...(typeof reason === 'string' && reason.trim().length > 0 ? { reason: reason.trim().slice(0, 1_000) } : {}) }
      try {
        await approvalRules.add({ ...candidate, effect })
      } catch (error) {
        return { ok: false, message: error instanceof Error ? error.message : 'That rule could not be saved. Answer the card as usual.' } as const
      }
      const answered = await answerApproval(decided, 'card-saving-a-rule')
      if (!answered) return { ok: false, message: 'The rule was saved, but this card had already been answered.' } as const
      return { ok: true, rules: await approvalRules.list() } as const
    })

    ipcMain.handle(MISSION_APPROVAL_DECIDE_CHANNEL, async (event, answer: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false } as const
      return { ok: await decideApproval(answer) } as const
    })
    /** A card answered from the window, or from the person's phone (0.775): one path, with every guard. */
    const decideApproval = async (answer: unknown): Promise<boolean> => {
      // A decision, or a question's answers -- see approvalAnswerFrom, and why
      // the answers used to be dropped right here.
      const decided = approvalAnswerFrom(answer)
      if (decided === undefined) return false
      // Whichever host is holding this id. A Codex approval lives in the
      // mission service's approval channel; a Claude Code connector permission
      // in the permission host; an Antigravity question in its mission
      // service. An id is minted by exactly one of them.
      // The host's own guard on "Always" (0.598): for a command that reaches other programs the
      // answer is applied as this once, whatever the window sent, and the record says so. A card
      // no longer in the raised map (evicted) is answered as it came.
      const raised = raisedApprovals.get(decided.approvalId)
      const enforced = raised === undefined ? { answer: decided } : enforcedAnswer(raised.request, decided)
      /*
       * AN ALWAYS IS REMEMBERED HERE (0.616), under the key the host put on the
       * request, and every later request of the run is decided with it after
       * the saved rules (answerByDecision). Kept before the answer goes back,
       * so a request the runtime raises the moment it is released finds it;
       * let go again if nothing was waiting for the answer.
       */
      const alwaysKey = raised?.request.alwaysKey
      const remembers = alwaysKey !== undefined && 'decision' in enforced.answer && enforced.answer.decision === 'approve-always'
      if (remembers) runAlways.add(raised!.request.runId, alwaysKey)
      const answered = await answerApproval(enforced.answer, undefined, enforced.note)
      if (remembers && !answered) runAlways.remove(raised!.request.runId, alwaysKey)
      return answered
    }

    const teammates = createTeammateStore({ rootDirectory: app.getPath('userData'), organizationPolicy })
    missionServiceForShutdown = codexMissions
    antigravityServiceForShutdown = antigravityMissions
    ledgerForShutdown = missionLedger
    workroomForShutdown = workroom

    /**
     * Who a mission is messaged to, resolved by the host from the roster it
     * reads itself. The renderer only names an id; an id that is nobody yields
     * a mission that belongs to nobody, never a guessed teammate.
     */
    // A teammate with "Own branch" on runs in its own worktree of the
    // folder's repository. Resolved here, once per start, because every
    // start for a teammate -- a person's message, a relay, a routine, a room
    // post -- builds its peer context through this one function.
    /*
     * `folder` is the conversation's own when it is not the window's (0.458):
     * an own-branch worktree is cut from THAT folder's repository.
     */
    const peerContextFor = async (teammateId: unknown, folder?: string): Promise<MissionPeerContext | undefined> => {
      const project = folder ?? (workspaceChosen ? workspacePath : undefined)
      const worktrees = project === undefined ? undefined : createWorktreeManager({ workspacePath: project, note: detail => note('worktree-add', detail) })
      if (typeof teammateId !== 'string' || teammateId.length === 0) return undefined
      let roster
      try {
        roster = await teammates.list()
      } catch {
        return undefined
      }
      const self = roster.find((entry) => entry.teammateId === teammateId)
      if (self === undefined) return undefined
      const entry = (teammate: PublicTeammate) => ({
        teammateId: teammate.teammateId,
        name: teammate.name,
        // What the runtime is told this teammate does: a Custom teammate's
        // own words, so the brief says "Wren (release manager)" rather than
        // "Wren (Custom)".
        role: roleLabelOf(teammate),
        kind: teammate.role,
        ...(teammate.route === undefined ? {} : { route: teammate.route })
      })
      let cwd: string | undefined
      let worktreeRefused: string | undefined
      /*
       * Where this teammate stands.
       *
       * Three cases and they compose. A teammate with its own folder works
       * there, and its own-branch worktree is cut from THAT folder's
       * repository rather than the project's -- a worktree of a repository
       * the teammate is not in would put it somewhere nobody asked for.
       * With no folder of its own it is the project folder, as before.
       *
       * The folder is not checked for existence here. `ensure` says so for
       * a worktree, and a run started in a folder that is gone fails with
       * the runtime's own words, which name the path. A silent fallback to
       * the project folder would be worse: the mission would succeed in the
       * wrong place.
       */
      const home = self.folder
      if (home !== undefined) cwd = home
      let repositoryRoot: string | undefined
      // Antigravity works in the folder it has open; a worktree would be one it has not.
      if (self.worktree === true && self.route?.runtime !== 'antigravity') {
        const manager = home === undefined ? worktrees : createWorktreeManager({ workspacePath: home, note: detail => note('worktree-add', detail) })
        if (manager !== undefined) {
          try {
            cwd = await manager.ensure(self)
            repositoryRoot = home ?? project
          } catch (error) {
            // Joined mid-sentence: "but the project folder is not...", not "but The".
            const why = error instanceof Error ? error.message : 'the worktree could not be made.'
            worktreeRefused = `${self.name} is set to work on its own branch, but ${why.charAt(0).toLowerCase()}${why.slice(1)}`
          }
        }
      }
      return {
        self: entry(self),
        others: roster.filter((other) => other.teammateId !== teammateId).map(entry),
        ...(cwd === undefined ? {} : { cwd }),
        ...(repositoryRoot === undefined ? {} : { repositoryRoot }),
        ...(self.connectors === undefined ? {} : { connectors: self.connectors }),
        ...(self.ownBrowser === true ? { browserProfile: { id: self.teammateId, name: self.name, hue: self.hue } } : {}),
        ...(worktreeRefused === undefined ? {} : { worktreeRefused })
      }
    }

    /**
     * Every folder a teammate has been pointed at, for the reveal allowlist.
     *
     * Read fresh rather than cached: a folder chosen a second ago has to be
     * openable, and an unreadable roster is an empty list rather than a
     * thrown reveal.
     */
    /*
     * The window's folder and every other folder worked in (0.458): a file a
     * conversation from another folder showed you opens like one from this.
     */
    const workedInFolders = async (): Promise<readonly string[]> => [
      ...(workspaceChosen ? [workspacePath] : []),
      ...(await folders.list().catch(() => [])).map((folder) => folder.path).filter((path) => path !== workspacePath)
    ]
    const teammateFolders = async (): Promise<readonly string[]> => {
      try {
        return (await teammates.list()).flatMap((teammate) => (teammate.folder === undefined ? [] : [teammate.folder]))
      } catch {
        return []
      }
    }
    // A file outside every folder above that a teammate's own record names (0.787, named-files.ts).
    const namedFile = (path: string): Promise<boolean> => namedByATeammate(missionLedger, path).catch(() => false)

    const antigravityStartData = (mission: {
      readonly runId: string
      readonly missionId: string
      readonly model: string
      readonly cliVersion: string | null
      readonly peerMessages: CodexMissionStartData['peerMessages']
      readonly peerDeliveryFailed: boolean
      readonly followsUp?: { readonly missionId: string; readonly runtimeThreadId: string }
    }): CodexMissionStartData => ({
      runId: mission.runId,
      missionId: mission.missionId,
      runtime: 'antigravity',
      model: mission.model,
      resolvedRouteId: 'antigravity:hub',
      cliVersion: mission.cliVersion,
      sandbox: 'workspace-write',
      peerMessages: mission.peerMessages,
      peerDeliveryFailed: mission.peerDeliveryFailed,
      ...(mission.followsUp === undefined ? {} : { followsUp: mission.followsUp })
    })

    // Ownership is recorded by the host, once, after a start succeeded -- so
    // the roster and the ledger cannot disagree about who a mission belongs
    // to because a renderer forgot to say.
    const assignOwner = async (teammateId: string | undefined, missionId: string): Promise<void> => {
      if (teammateId === undefined) return
      await teammates.assignMission(teammateId, missionId).catch(() => undefined)
    }
    /*
     * A person's own turn in a teammate's hub moves the hub along with it.
     *
     * The hub records its NEWEST turn, and the relay continues from there.
     * When the person follows up in the hub -- which is what the hub is for
     * -- the next relayed reply has to come after what they said, not
     * beside it. Only when the follow-up IS the hub's newest turn: a
     * follow-up on some other conversation is not the hub's business.
     */
    /** The tip of the conversation a rewind replaces, when the start is one (0.498). */
    const rewindTip = (payload: { readonly rewind?: unknown }): string | undefined => {
      const rewind = payload.rewind
      if (typeof rewind !== 'object' || rewind === null) return undefined
      const tip = (rewind as { readonly tip?: unknown }).tip
      return typeof tip === 'string' && tip.length > 0 ? tip : undefined
    }
    /** What the turns a rewind set aside sent and nobody read: never delivered (0.512, set-aside-messages.ts). */
    const retireSetAside = async (payload: { readonly rewind?: unknown }): Promise<void> => {
      const rewind = payload.rewind
      const listed = typeof rewind === 'object' && rewind !== null ? (rewind as { readonly setAside?: unknown }).setAside : undefined
      // Left waiting on a failure: a stale message delivered is recoverable, a workroom broken is not.
      await retireSetAsideMessages(workroom, listed).catch(() => undefined)
    }
    const advanceHub = async (
      teammateId: string | undefined,
      followUpOf: string | undefined,
      missionId: string
    ): Promise<void> => {
      if (teammateId === undefined || followUpOf === undefined) return
      const own = (await teammates.list().catch(() => [])).find((entry) => entry.teammateId === teammateId)
      if (own?.hubMissionId !== followUpOf) return
      await teammates.rememberHub(teammateId, missionId).catch(() => undefined)
    }
    // A PERSON starting a teammate on a route is what makes it theirs. A run
    // the relay starts for them never re-records it, so a fallback onto the
    // sender's route cannot quietly become the recipient's own.
    //
    // Only a teammate who has NO route yet (0.552). Colin, 10/02: "if i have
    // a teammate set to a certain model i should be able to run a sepate chat
    // with a new model without assigning that as their new model". Every run
    // rewrote it, so trying a model in one chat made it the teammate's. As in
    // Claude Code, a conversation keeps its own model and the default changes
    // only where it is set: the teammate's profile (Edit teammate's Model row).
    const rememberRoute = async (
      teammateId: string | undefined,
      route: { readonly runtime: MissionRuntimeId; readonly model: string; readonly mode: MissionMode; readonly effort?: string; readonly account?: string }
    ): Promise<void> => {
      if (teammateId === undefined) return
      const own = (await teammates.list().catch(() => [])).find((entry) => entry.teammateId === teammateId)
      if (own === undefined || own.route !== undefined) return
      await teammates.rememberRoute(teammateId, route).catch(() => undefined)
    }

    // Teammates replying to each other. Off unless the workspace switched it
    // on; every hop is a run the service starts like any other, under the
    // recipient's name, on the sender's route.
    relay = createRelay({
      enabled: async () => (await teammates.readSettings()).relay === true,
      hopCap: async () => (await teammates.readSettings()).relayHopCap,
      // A2.4: every run a room post started is one exchange, so the budget
      // bounds the post -- not each member's own corner of it.
      exchangeOf: async (missionId) => exchangeOfRoomPost(await rooms.list(), missionId),
      peerContextFor,
      // A2.1: an answer not written back is brought to the teammate who asked.
      finalReplyOf: async (missionId) => {
        const recovered = await missionLedger.getMission(missionId)
        if (recovered === undefined) return undefined
        const tracker = createTranscriptTracker()
        tracker.track(recovered.events)
        return tracker.latestFinal
      },
      howEnded: async (missionId) => {
        const recovered = await missionLedger.getMission(missionId)
        return recovered === undefined ? undefined : howTurnEnded(recovered.events)
      },
      post: (input) => workroom.post(input),
      cursorHoldsReadOnly: () => cursorCanEnforceReadOnly(process.platform),
      start: async (input) => {
        // The app route only when Antigravity CLI is not installed (0.540); the CLI runs as every other runtime does.
        if (input.runtime === 'antigravity' && antigravityProbe.cliPath() === undefined) {
          try {
            const mission = await antigravityMissions.start(input.prompt, input.peer, {
              ...(input.model === undefined ? {} : { model: input.model }),
              ...(input.followUpOf === undefined ? {} : { followUpOf: input.followUpOf }),
              relay: input.relay
            })
            return { ok: true, data: antigravityStartData(mission) } as const
          } catch (error) {
            // A2.19: busy is BUSY on Antigravity too, so the relay holds the
            // reply instead of dropping it (antigravityStartRefusal).
            return antigravityStartRefusal(error)
          }
        }
        return codexMissions.start(
          input.prompt,
          input.runtime,
          input.mode,
          // M10: the effort as well as the model, which this used to drop.
          { ...(input.model === undefined ? {} : { model: input.model }), ...(input.account === undefined ? {} : { account: input.account }), ...(input.effort === undefined ? {} : { effort: input.effort }) },
          sendToWindow,
          undefined,
          input.peer,
          input.followUpOf,
          input.relay
        )
      },
      assignOwner: (teammateId, missionId) => assignOwner(teammateId, missionId),
      /*
       * The teammate's hub: where a reply with no exchange of its own goes.
       *
       * Answered from the roster, and checked against the ledger before it is
       * handed over: a hub whose newest turn was deleted is begun again
       * rather than refused, because the start would otherwise fail with
       * "its earlier mission is not in the ledger" -- true, and no help.
       */
      hubOf: async (teammateId) => {
        const own = (await teammates.list()).find((entry) => entry.teammateId === teammateId)
        const hub = own?.hubMissionId
        if (hub === undefined) return undefined
        const kept = await missionLedger.getMission(hub).catch(() => undefined)
        return kept === undefined ? undefined : hub
      },
      rememberHub: async (teammateId, missionId, began) => {
        await teammates.rememberHub(teammateId, missionId)
        // The row's name, given once. A hub's root prompt is whatever the
        // first peer message happened to be -- the sidebar would title the
        // whole conversation with one teammate's aside to another. Named
        // like anything a person names, so it can be renamed like one.
        if (!began) return
        const own = (await teammates.list().catch(() => [])).find((entry) => entry.teammateId === teammateId)
        if (own === undefined) return
        await teammates.renameMission(missionId, `${own.name}'s replies`).catch(() => undefined)
      },
      // The one question a held reply has to ask before it starts: is there
      // still anything to show them? Whatever a teammate has not read rides
      // along on the next run whoever starts it, so a person who messaged
      // them in the meantime already delivered the message.
      stillWaiting: async (teammateId) => (await workroom.unread(teammateId, 1)).messages.length > 0,
      mayInterrupt: async () => (await teammates.readSettings()).interrupt === true,
      /*
       * Stop whatever this teammate is running, whichever transport owns it.
       *
       * Only ever stops. The waiting message is started by the relay's own
       * held-message path when the run ends, which is the path every other
       * waiting message takes -- so an interruption cannot become a second
       * way to start a mission, and the hop cap and the relay switch bound it
       * because it never goes near either.
       */
      /*
       * Write an ending into the mission's own record.
       *
       * A relay notice was a live update only, so a person watching another
       * conversation when an exchange stopped never learned it had, and
       * reopening the thread later showed nothing -- it simply appeared to
       * stop for no reason. The host already raises `host.*` diagnostics into
       * a mission this way (`host.shared_workspace`, disk-observation.ts);
       * this is the same shape.
       *
       * Appending AFTER a run's terminal event is safe: `phaseFor` scans
       * backwards for any terminal event, so a completed mission stays
       * completed. Checked before this was written rather than after.
       */
      // One at a time: each reads the tail it appends after, and the relay
      // fires several without waiting -- two at once read the same tail and
      // the second ending note was refused and lost (B4 lead).
      note: oneAtATime(async ({ missionId, message }: { readonly missionId: string; readonly message: string }) => {
        const mission = await missionLedger.getMission(missionId).catch(() => undefined)
        if (mission === undefined) return
        const last = mission.events.at(-1)
        // The ledger requires contiguous sequences, and this is the only
        // writer once a run is over -- so the recorded tail is the truth.
        const sequence = (last?.sequence ?? 0) + 1
        await missionLedger
          .appendEvents(missionId, [
            {
              id: `${mission.metadata.runId}:relay:${String(sequence)}`,
              runId: mission.metadata.runId,
              missionId,
              sequence,
              type: 'adapter.diagnostic',
              occurredAt: new Date().toISOString(),
              sourceAdapter: last?.sourceAdapter ?? mission.metadata.runtime,
              payload: {
                level: 'info',
                code: 'host.relay_ended',
                message,
                terminal: false,
                evidence: { redacted: true }
              }
            } as NormalizedRuntimeEvent
          ])
          .catch(() => undefined)
      }),
      stopWorkOf: async (teammateId) => {
        const codexRun = codexMissions.runIdOwnedBy(teammateId)
        if (codexRun !== undefined) return codexMissions.cancel(codexRun).ok
        const antigravityRun = antigravityMissions.runIdOwnedBy(teammateId)
        if (antigravityRun !== undefined) return antigravityMissions.cancel(antigravityRun)
        return false
      },
      // A2.10: a running Codex (app-server) or Claude Code turn is shown the message at its next step instead.
      steerWorkOf: async (teammateId, text) => {
        const codexRun = codexMissions.runIdOwnedBy(teammateId)
        return codexRun === undefined ? false : codexMissions.steer(codexRun, text)
      },
      notify: sendToWindow
    })

    /*
     * WHEN A PLAN RUNS OUT (0.774, carry-on.ts): the turn is carried on with another agent, as a reply on that agent
     * -- the same start a teammate's reply takes, recorded as Locust's (`carry-on`), never as the person's words.
     * Without a press only for a turn the person sent from the chat box, which `sentByPerson` holds for this session.
     */
    const sentByPerson = new Set<string>()
    const rememberSentByPerson = (missionId: string): void => {
      sentByPerson.add(missionId)
      while (sentByPerson.size > 500) {
        const oldest = sentByPerson.values().next().value
        if (oldest === undefined) break
        sentByPerson.delete(oldest)
      }
    }
    // Said in the stopped turn's thread now, and kept in its record for when it is opened again (as the relay's are).
    const noteCarryOn = oneAtATime(async ({ missionId, message }: { readonly missionId: string; readonly message: string }) => {
      const mission = await missionLedger.getMission(missionId).catch(() => undefined)
      if (mission === undefined) return
      sendToWindow({ kind: 'relay-notice', runId: mission.metadata.runId, missionId, message, level: 'warning' })
      const last = mission.events.at(-1)
      const sequence = (last?.sequence ?? 0) + 1
      await missionLedger
        .appendEvents(missionId, [
          {
            id: `${mission.metadata.runId}:carry-on:${String(sequence)}`,
            runId: mission.metadata.runId,
            missionId,
            sequence,
            type: 'adapter.diagnostic',
            occurredAt: new Date().toISOString(),
            sourceAdapter: last?.sourceAdapter ?? mission.metadata.runtime,
            payload: { level: 'warning', code: 'host.carry_on', message, terminal: false, evidence: { redacted: true } }
          } as NormalizedRuntimeEvent
        ])
        .catch(() => undefined)
    })
    carryOn = createCarryOn({
      ledger: missionLedger,
      setting: async () => (await teammates.readSettings()).planRunsOut ?? undefined,
      sentByPerson: (missionId) => sentByPerson.has(missionId),
      note: (missionId, message) => void noteCarryOn({ missionId, message }).catch(() => undefined),
      start: async (input) => {
        // Antigravity's app route records no starter, so its turn would read as the person's words.
        if (input.runtime === 'antigravity' && antigravityProbe.cliPath() === undefined) {
          return { ok: false, message: 'Antigravity carries work on only through its command line, and it is not installed.' }
        }
        const owner = (await teammates.missionOwners().catch(() => ({} as Readonly<Record<string, string>>)))[input.followUpOf]
        const peer = owner === undefined ? undefined : await peerContextFor(owner)
        const wasHub = owner !== undefined && (await teammates.list().catch(() => [])).find((entry) => entry.teammateId === owner)?.hubMissionId === input.followUpOf
        const response = await codexMissions.start(
          input.prompt,
          input.runtime,
          input.mode,
          input.model === undefined ? {} : { model: input.model },
          sendToWindow,
          undefined,
          peer,
          input.followUpOf,
          undefined,
          { kind: 'carry-on', from: input.from }
        )
        if (!response.ok) return { ok: false, message: response.error.message }
        await assignOwner(owner, response.data.missionId)
        await advanceHub(owner, input.followUpOf, response.data.missionId)
        sendToWindow({
          kind: 'mission-started',
          runId: response.data.runId,
          missionId: response.data.missionId,
          ...(owner === undefined ? {} : { teammateId: owner }),
          prompt: input.prompt,
          data: response.data,
          startedBy: { kind: 'carry-on', from: input.from },
          ...(wasHub ? { hubMissionId: response.data.missionId } : {})
        })
        return { ok: true, missionId: response.data.missionId }
      }
    })
    ipcMain.handle(CARRY_ON_CHANNEL, async (event, missionId: unknown, route: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'Only Locust’s own window can carry a turn on.' }
      const chosen = parsedCarryOnRoute(route)
      if (typeof missionId !== 'string' || missionId.length === 0 || chosen === undefined) return { ok: false, message: 'That is not a turn and an agent Locust knows.' }
      return carryOn?.carry(missionId, chosen) ?? { ok: false, message: 'Locust is still starting.' }
    })

    // Routines replay through the same start path a teammate's reply uses,
    // so a replayed step is a real mission on the teammate's own route, in
    // its own ledger, recorded as started by the routine.
    const routines = createRoutineStore({ rootDirectory: app.getPath('userData') })
    let routineFolderChosen: (path: string) => boolean = () => false
    const rooms = createRoomStore({ rootDirectory: app.getPath('userData') })
    // Comparisons (0.441, shared/compare.ts), kept beside the rooms, outside the ledger.
    const compares = createCompareStore({ rootDirectory: app.getPath('userData'), copyRoot: compareRoot() })
    // Tune lessons (0.810, tune-lesson-store.ts), at most 20 per teammate, read only by /tune and Edit teammate.
    const tuneLessons = createTuneLessonStore({ rootDirectory: app.getPath('userData') })
    // Folder watchers (0.522): routines that run on a new file, polled, inside the project folder.
    const fileArrivals = createFileArrivals({ get projectFolder() { return workspacePath } })
    turnWrote = (workspace, paths) => fileArrivals.wrote(workspace, paths)
    let hookRelay: HookRelay | undefined
    const routineHooks = createRoutineHooks({
      secrets: { available: () => safeStorage.isEncryptionAvailable(), encrypt: (text) => safeStorage.encryptString(text), decrypt: (cipher) => safeStorage.decryptString(cipher) },
      file: join(app.getPath('userData'), ROUTINE_HOOKS_FILE), relay: () => hookRelay,
      routines: () => routines.list(),
      lost: async (id, count, at) => { await routines.recordHookLoss(id, count, at); sendToWindow({ kind: 'routine-recovery-changed' }) },
      tick: () => { if (workspaceChosen) void routineRunner?.tick(new Date()).catch(() => undefined) }
    })
    const syncHooks = (): void => { void routineHooks.sync().catch(() => note('routine', 'The called links could not be collected. They will be tried again when the relay connects.')) }
    const routineCopies = createRoutineCopies()
    /*
     * The folder's own check (0.534): this folder's command from Settings, the one the after-edits check runs -- never a
     * command the project names -- run in `cwd`, with the end of what it printed. A routine's goal and Scout (0.815)
     * both read it.
     */
    const runFolderCheck = async (cwd: string): Promise<{ readonly command: string; readonly passed: boolean; readonly tail: readonly string[] } | undefined> => {
      if (!workspaceChosen) return undefined
      const command = (await teammates.readSettings()).checkCommands?.[workspaceIdFor(workspacePath)]
      if (command === undefined || command.length === 0) return undefined
      const run = await runCheckCommand(command, cwd)
      const tail = run.error !== undefined
        ? [run.error]
        : run.timedOut
          ? ['The check ran past its time limit and was stopped.']
          : comparableLines(run.output).slice(-25).map((line) => (line.length > 300 ? `${line.slice(0, 299)}…` : line))
      return { command, passed: run.error === undefined && !run.timedOut && run.exitCode === 0, tail }
    }
    routineRunner = createRoutineRunner({
      hooks: routineHooks,
      folderChosen: (path) => routineFolderChosen(path),
      arrivals: fileArrivals,
      // A routine that works in a copy (0.533): made from the folder the window is in.
      copies: routineCopies,
      // A step that waits for an OK (0.840) is told as a question is: Windows, the phone, the chat apps.
      asksFirst: (name, routineName, step) => {
        attention.decisionAsked(name, `Routine "${routineName}": step ${String(step)} waits for your OK. Go ahead under Routines.`)
        phoneAlerts.routineOk(name)
      },
      folderNow: () => workspacePath,
      // A standing goal's check (0.534): the end of what it printed goes to the teammate asked to fix it.
      goalCheck: (cwd) => runFolderCheck(cwd),
      // Read live (0.458): the folder the window is in now.
      openedAt: appOpenedAt,
      get workspaceId() {
        return memoryWorkspaceId
      },
      // M15: where a routine was made; for an older one, where it was learned.
      homeOf: async (routine) => {
        if (routine.workspaceId !== undefined) return routine.workspaceId
        const first = routine.learnedFrom[0]
        if (first === undefined) return undefined
        return (await missionLedger.getMission(first).catch(() => undefined))?.metadata.workspaceId
      },
      routines,
      peerContextFor,
      start: (input): Promise<CodexMissionStartResponse> => {
        // The login-item smoke records that a due routine dispatched, and
        // sends nothing to a model. A packaged build never takes this path.
        const sink = app.isPackaged ? undefined : process.env.LOCUST_SCHEDULE_SINK
        if (sink !== undefined && sink.length > 0) {
          mkdirSync(dirname(sink), { recursive: true })
          appendFileSync(sink, `${input.startedBy.routineId}\n`, 'utf8')
          return Promise.resolve({
            ok: true,
            data: {
              missionId: 'mission_schedule_sink',
              runId: 'run_schedule_sink',
              runtime: input.runtime,
              model: input.model ?? '',
              resolvedRouteId: 'schedule-sink',
              cliVersion: null,
              sandbox: 'read-only',
              peerMessages: [],
              peerDeliveryFailed: false
            }
          })
        }
        return codexMissions.start(
          input.prompt,
          input.runtime,
          input.mode,
          {
            ...(input.model === undefined ? {} : { model: input.model }),
            // A routine replays on a STORED route, which now carries the level
            // it was taught with. Without this one saved at `high` quietly
            // replayed at the runtime's default.
            ...(input.account === undefined ? {} : { account: input.account }), ...(input.effort === undefined ? {} : { effort: input.effort })
          },
          sendToWindow,
          undefined,
          input.peer,
          input.followUpOf,
          undefined,
          input.startedBy,
          undefined,
          // In the routine's copy (0.533): a slot keyed by the teammate, so one run at a time
          // stays one run at a time, and its end never commits the teammate's own branch.
          input.cwd === undefined ? undefined : { key: input.peer.self.teammateId, cwd: input.cwd }
        )
      },
      assignOwner: (teammateId, missionId) => assignOwner(teammateId, missionId),
      phaseOf: async (missionId) => (await missionLedger.getMission(missionId))?.phase,
      // Whichever transport owns it. The ledger cannot say "still going" --
      // a mission with no terminal event reads as `interrupted` either way.
      isLive: (missionId) => codexMissions.hasMission(missionId) || antigravityMissions.hasMission(missionId),
      /*
       * Whether that turn ended by asking the person something.
       *
       * The same three steps `attention-reader.ts` already takes to decide it
       * has a decision to announce: recover the mission, rebuild the last
       * FINAL assistant message, look for a decision block. Reusing that path
       * rather than inventing a second one is the point -- two readers
       * disagreeing about whether a turn asked a question is how the card and
       * the routine would end up on different sides of one fact.
       */
      askedAQuestion: async (missionId) => {
        try {
          const recovered = await missionLedger.getMission(missionId)
          if (recovered === undefined) throw new Error('Routine mission receipt unavailable')
          const tracker = createTranscriptTracker()
          tracker.track(recovered.events)
          const text = tracker.latestFinal
          return text === undefined ? false : parseDecision(text) !== undefined
        } catch (error) {
          // Recovery must distinguish an unreadable answer from "no question".
          throw error
        }
      },
      // A scheduled routine waits for any live run of the teammate's, whoever started it.
      teammateBusy: async (teammateId) => {
        const owners = await teammates.missionOwners()
        const live = [...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()]
        return live.some((missionId) => owners[missionId] === teammateId)
      },
      // A hand-off chain (0.435): the teammate a step is handed to runs it on
      // their own route, and is given the step before's final answer.
      routeOf: async (teammateId) => (await teammates.list()).find((teammate) => teammate.teammateId === teammateId)?.route,
      replyOf: async (missionId) => {
        const recovered = await missionLedger.getMission(missionId)
        if (recovered === undefined) return undefined
        const tracker = createTranscriptTracker()
        tracker.track(recovered.events)
        return tracker.latestFinal
      },
      notify: (update) => {
        if (update.kind === 'mission-started' && update.startedBy?.kind === 'routine') routineLastStep.set(update.startedBy.routineId, update.missionId)
        sendToWindow(update)
      }
    })
    // Scheduled routines: one tick a minute, the first after the runtimes
    // have had a moment to be discovered. Only with a project folder chosen
    // -- a run needs one, and a routine started into nothing would fail and
    // be held off an hour for a reason the person never saw.
    const tickRoutines = (): void => {
      syncHooks()
      if (!workspaceChosen || routineRunner === undefined) return
      void routineRunner.tick(new Date()).catch(() => undefined)
    }
    // Warmed here rather than at import: it spawns Claude Code, and doing
    // that before the window exists would put a health check in front of the
    // first paint.
    void connectorReader.refresh().catch(() => undefined)
    const firstRoutineTick = setTimeout(tickRoutines, ROUTINE_FIRST_TICK_MS)
    firstRoutineTick.unref()
    const routineTicks = setInterval(tickRoutines, ROUTINE_TICK_MS)
    routineTicks.unref()
    /*
     * The watched folders (0.522): looked at every POLL_MS; a routine with a
     * settled new file starts on a tick at once rather than within the minute.
     */
    const pollWatchedFolders = (): void => {
      if (!workspaceChosen || routineRunner === undefined) return
      void (async () => {
        const all = await routines.list().catch(() => [])
        const watching = all.flatMap((routine) =>
          routine.schedule?.kind === 'files' && (routine.workspaceId === undefined || routine.workspaceId === memoryWorkspaceId)
            ? [{ routineId: routine.routineId, folder: routine.schedule.folder }]
            : []
        )
        if (watching.length === 0) return
        // A teammate at work: what settles now may be its own, held until its turn says what it wrote.
        await fileArrivals.poll(watching, Date.now(), codexMissions.liveMissionIds().length > 0 || antigravityMissions.liveMissionIds().length > 0)
        if (watching.some((entry) => fileArrivals.ready(entry.routineId).length > 0)) tickRoutines()
      })().catch(() => undefined)
    }
    const folderPolls = setInterval(pollWatchedFolders, POLL_MS)
    folderPolls.unref()
    app.once('before-quit', () => {
      clearTimeout(firstRoutineTick)
      clearInterval(routineTicks)
      // Every discovery probe still out: a hung shim's sleep outlived the
      // app otherwise (Fable, pass 1: ten processes after a close mid-sweep
      // on Linux; zero after a settled one). The timer was the only caller
      // of the kill, and there is no timer once the app has gone.
      probeRunner.dispose()
      sessionProbeRunner.dispose()
    })
    // A run that ended waiting for the person -- a question card, an account
    // limit -- is said to the desk the way an approval is (parity row 62).
    attentionReader = createAttentionReader({
      ledger: missionLedger,
      teammates,
      attention: {
        decisionAsked: (name, question) => { phoneAlerts.approval(name, 'question'); return attention.decisionAsked(name, question) },
        limitHit: (name, runtime, message) => {
          phoneAlerts.limit(name, isMissionRuntime(runtime) ? runtimeDisplayName(runtime) : runtime)
          return attention.limitHit(name, isMissionRuntime(runtime) ? runtimeDisplayName(runtime) : runtime, message)
        }
      }
    })
    memoryReader = createMemoryReader({
      memories,
      ledger: missionLedger,
      teammates,
      get workspaceName() {
        return memoryWorkspaceName
      },
      // A reply from a conversation in another folder names THAT folder (0.458).
      workspaceNameOf: async (id: string) => (await folders.list()).find((folder) => folder.id === id)?.name,
      notify: sendToWindow,
      // About you (0.424): a teammate's line waits for the person, once --
      // not again while it is waiting, and not if the note already says it.
      suggestAboutYou: async (text, by) => {
        const settings = await teammates.readSettings()
        const waiting = settings.aboutYouSuggestions ?? []
        const said = (line: string): string => line.trim().toLowerCase()
        if ((settings.aboutYou ?? '').toLowerCase().includes(said(text)) || waiting.some((entry) => said(entry.text) === said(text))) return false
        const next = [...waiting, { id: `ays_${randomUUID()}`, text, by, at: new Date().toISOString() }]
        await teammates.writeSettings({ aboutYouSuggestions: next.slice(-MAX_ABOUT_YOU_SUGGESTIONS) })
        return true
      }
    })
    roomTasks = createRoomTasks({
      rooms,
      ledger: missionLedger,
      teammates,
      /*
       * Start one member who has been waiting for a slot.
       *
       * Same path the post itself uses, so a queued member is briefed with
       * the board as it stands NOW and runs on their own route -- they are
       * answering the same post, just later. The window is told the same
       * way the post tells it, or the run would be live with no row.
       *
       * `startRoomMember` is declared further down this scope; this closure
       * only reads it when a run ends, long after.
       */
      startQueued: async (room, postId, teammateId) => {
        const post = room.posts.find((entry) => entry.postId === postId)
        if (post === undefined) return 'refused'
        // A roster that cannot be read cannot say who this member is --
        // this time. Asked again at the next run end, not refused for good.
        let roster: Awaited<ReturnType<typeof teammates.list>>
        try {
          roster = await teammates.list()
        } catch {
          return 'busy'
        }
        const attempt = await startRoomMember(room, teammateId, post, roster)
        if (!attempt.ok) return attempt.retryable ? (attempt.pool === true ? 'no-slot' : 'busy') : { refused: attempt.message }
        sendToWindow({
          kind: 'mission-started',
          runId: attempt.data.runId,
          missionId: attempt.data.missionId,
          teammateId,
          prompt: post.text,
          data: attempt.data,
          startedBy: { kind: 'room', roomId: room.roomId, postId }
        })
        return { missionId: attempt.data.missionId }
      },
      notify: (update) => {
        sendToWindow(update)
        // Also to the desk, when the person is elsewhere: gathered per room
        // so three teammates finishing together are one toast, not three.
        if (update.kind === 'room-changed') {
          attention.roomChanged({ roomId: update.roomId, roomName: update.roomName, message: update.message })
        }
      }
    })

    ipcMain.handle(RUNTIME_DISCOVERY_CHANNEL, async (event, fresh: unknown, only: unknown) => {
      // Who is asking, FIRST: this used to drop every cached answer before
      // it checked, so a frame it then refused had still cleared them.
      const owner = BrowserWindow.fromWebContents(event.sender)
      if (!owner || !event.senderFrame || event.senderFrame.parent !== null) {
        return {
          ok: false,
          error: {
            code: 'DISCOVERY_FAILED',
            message: 'Local runtime discovery could not complete.'
          }
        } as const
      }
      /*
       * `fresh` DROPS EVERY CACHED ANSWER, INCLUDING THE ONE ABOUT npm.
       *
       * npm is probed once and remembered for the session, and `npmSeen` was
       * cleared only by the install channel. So a person whose npm hangs got
       * the wrong sentence on the first screen and no way to correct it --
       * Check again asked the CLIs again and never re-asked npm, and the
       * screen's account of the machine stayed stale until they pressed
       * Install (Fable, pass 2, finding 3b: "clearing npmSeen in
       * onCheckAgain's path would let the one repair the app offers repair
       * this too").
       *
       * They were right and my hesitation was wrong in a specific way. I
       * wanted a measurement before adding a five-second probe to a button
       * whose purpose is to be fast -- but the five seconds only ever land on
       * the machine whose npm hangs, which is precisely the machine that
       * needs re-asking, and on every other machine npm answers in
       * milliseconds. The cost falls exactly where the repair is wanted.
       *
       * Only on an explicit ask. The automatic sweeps pass nothing and keep
       * the cache, so the ordinary re-check is as cheap as it ever was.
       */
      if (fresh === true) {
        npmSeen = undefined
        npmAnswered = true
        discoveryCache = undefined
        // AND the service's own ten-second answer, which this used to leave
        // standing: Check again pressed within ten seconds of a sweep got
        // that sweep back without asking anything (main-process audit,
        // 2026-09-22).
        runtimeDiscovery.invalidate()
      }
      /*
       * ASK AGAIN ABOUT THE ONES THAT ARE NOT READY, NOT ABOUT EVERYTHING.
       *
       * Returning to the window re-swept every runtime whenever one was not
       * ready -- so one signed-out CLI meant ~16 processes on every alt-tab,
       * all session (main-process audit, 2026-09-22; Muse sat signed out on
       * Colin's machine all afternoon). The window now names the runtimes it
       * is waiting on and only those are asked; the rest keep their answer.
       */
      const named = Array.isArray(only) ? only.filter((id): id is string => typeof id === 'string').slice(0, 16) : []
      const cached = discoveryCache
      if (fresh !== true && named.length > 0 && cached !== undefined) {
        try {
          await runtimeFactsLoaded
          /*
           * Antigravity is asked only if the held answer is not already a real
           * one (a late answer is patched into the held list); an agent whose
           * check is still going is joined, not asked again; and none of them
           * holds up the others (askRuntimes, sweep-settle.ts).
           */
          const wanted = new Set(named)
          if (cached.value.find((entry) => entry.id === 'antigravity')?.readiness === 'ready') wanted.delete('antigravity')
          for (const id of [...wanted]) if (outstanding.has(id)) wanted.delete(id)
          const answers = new Map<string, RuntimeDiscovery>()
          if (wanted.size > 0) {
            for (const entry of await askRuntimes(wanted)) answers.set(entry.id, entry)
          }
          const latest = discoveryCache ?? cached
          discoveryCache = { at: Date.now(), value: latest.value.map((entry) => answers.get(entry.id) ?? entry) }
          startReadiness.swept([...answers.values()].filter((entry) => !outstanding.has(entry.id)), Date.now())
          runtimeDiscovery.invalidate()
          discoveryLog.emit({ kind: 'reasked', ids: [...answers.keys()], at: Date.now() })
        } catch {
          // A failed partial ask changes nothing; the answer below is the held one.
        }
      }
      const answer = await runtimeDiscovery.get()
      // Cursor's connectors, read in the background as soon as the WINDOW finds Cursor (0.693): the
      // work-path sweep runs only when a turn starts, so warming there left the first turn waiting.
      if (answer.ok && answer.data.runtimes.some((runtime) => runtime.id === 'cursor' && runtime.ready)) warmCursorConnectors()
      // And OpenCode's first-start setup, for the same reason (0.780): as the window finds it, not when a turn starts.
      if (answer.ok && process.env.LOCUST_HIDE_RUNTIMES !== '1' && answer.data.runtimes.some((runtime) => runtime.id === 'opencode' && runtime.ready && runtime.version?.startsWith('1.'))) void openCodeFirstStart.ensure()
      return answer
    })

    // Every teammate channel validates its sender the same way the mission
    // channels do: a top-level frame of a window this process owns, never a
    // subframe. The roster is local data, but it is still a write surface.
    const fromOwnWindow = (event: Electron.IpcMainInvokeEvent): boolean =>
      BrowserWindow.fromWebContents(event.sender)?.webContents === event.sender
      && event.senderFrame !== null
      && event.senderFrame.parent === null

    // Constructing the host starts no I/O, download, microphone, timer, or sidecar.
    const voiceSettings = createVoiceSettingsStore(app.getPath('userData'), {
      available: () => safeStorage.isEncryptionAvailable(),
      encrypt: (text) => safeStorage.encryptString(text),
      decrypt: (bytes) => safeStorage.decryptString(bytes)
    })
    const voice = createVoiceHost(join(app.getPath('userData'), 'voice'), { store: voiceSettings })
    voiceForShutdown = voice
    const fromVoiceWindow = (event: Electron.IpcMainInvokeEvent): boolean =>
      fromOwnWindow(event) && BrowserWindow.fromWebContents(event.sender) === approvalWindow
    const voiceRefused = { ok: false, message: 'Voice typing could not start here. Your message is unchanged.' } as const
    ipcMain.handle(VOICE_SETTINGS_READ, (event) => fromVoiceWindow(event) ? voiceSettings.settings() : { mode: 'fast', hasKey: false, openaiConsent: false })
    ipcMain.handle(VOICE_SETTINGS_SAVE, async (event, change: unknown) => {
      if (!fromVoiceWindow(event)) return voiceRefused
      await voice.cancel()
      return voiceSettings.save(change)
    })
    ipcMain.handle(VOICE_OPENAI_CONSENT, (event) => fromVoiceWindow(event) ? voiceSettings.consent() : voiceRefused)
    ipcMain.handle(VOICE_READY, (event, mode: unknown) => fromVoiceWindow(event) ? voice.ready(mode) : false)
    ipcMain.handle(VOICE_DOWNLOAD, (event, mode: unknown) => fromVoiceWindow(event) ? voice.download((percent) => {
      if (!event.sender.isDestroyed()) event.sender.send(VOICE_PROGRESS, percent)
    }, mode) : voiceRefused)
    ipcMain.handle(VOICE_TRANSCRIBE, (event, wav: unknown, mode: unknown) => fromVoiceWindow(event) ? voice.transcribe(wav, mode) : voiceRefused)
    ipcMain.handle(VOICE_CANCEL, (event) => fromVoiceWindow(event) ? voice.cancel() : undefined)

    /*
     * A VOICE CALL (0.737): ChatGPT's voice, placed by Codex as itself (shared/voice-call.ts). The window
     * holds the microphone and the WebRTC connection; this side only starts Codex and passes the offer and
     * the answer, and the lines said. One call at a time, ended with the window or the app.
     */
    let voiceCallWindow: Electron.WebContents | undefined
    /** Locust's own add-on server, a private copy for each call (0.739); set once the tools exist, below. */
    let voiceCallAddOn: { readonly open: () => Promise<VoiceAddOn | undefined>; readonly close: () => Promise<void> } | undefined
    // The conversation a call with a teammate may continue (0.746), while that call is on.
    let voiceCallHere: VoiceCallTarget | undefined
    // The keys that start Talk from any app (0.755); made below, once the calls are.
    let talkShortcut: TalkShortcut | undefined
    // Talk's voice (0.745), kept in the profile: a preference, picked again in a moment after a restore.
    const voiceCallVoiceFile = join(app.getPath('userData'), 'voice-call.json')
    const voiceCallVoice = async (): Promise<ChatGptVoice> => {
      const kept = (await talkSettings()).voice
      return isChatGptVoice(kept) ? kept : DEFAULT_CHATGPT_VOICE
    }
    /** Talk's settings in that file: its voice, and its keys (0.755). One unread is none. */
    async function talkSettings(): Promise<Record<string, unknown>> {
      try {
        const parsed: unknown = JSON.parse(await readFile(voiceCallVoiceFile, 'utf8'))
        return typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed) ? parsed as Record<string, unknown> : {}
      } catch {
        return {}
      }
    }
    /** One of them changed, the other kept: the voice was the file's only setting until the keys joined it. */
    let talkWriting: Promise<unknown> = Promise.resolve()
    function keepTalkSetting(change: Readonly<Record<string, unknown>>): Promise<void> {
      const write = talkWriting.then(async () => { await writeFile(voiceCallVoiceFile, `${JSON.stringify({ ...(await talkSettings()), ...change })}\n`, 'utf8') })
      talkWriting = write.catch(() => undefined)
      return write
    }
    const voiceCalls = createVoiceCalls({
      policyRefusal: (target, preview) => policyStartRefusal({ runtime: 'codex', mode: preview === true ? 'ask' : 'approve-each' }, target?.teammateId),
      voice: voiceCallVoice,
      // Talk pressed in a teammate's conversation is a call with that teammate (0.746): its name, role, runtime and voice.
      teammate: async (target) => {
        const list = await listTeammates()
        const found = list.ok ? list.data.teammates.find((teammate) => teammate.teammateId === target.teammateId) : undefined
        if (found === undefined) return undefined
        // The conversation must be this teammate's: a call never continues anyone else's.
        const owner = target.conversationId === undefined ? undefined : (await teammates.missionOwners().catch(() => ({}) as Record<string, string>))[target.conversationId]
        const conversationId = owner === found.teammateId ? target.conversationId : undefined
        const role = found.roleTitle ?? (found.role === 'Custom' ? undefined : found.role)
        return {
          teammateId: found.teammateId,
          name: found.name,
          ...(role === undefined ? {} : { role }),
          runtime: found.route === undefined ? 'its own AI agent' : runtimeDisplayName(found.route.runtime),
          ...(conversationId === undefined ? {} : { conversationId }),
          ...(found.voice === undefined ? {} : { voice: found.voice })
        }
      },
      onTarget: (target) => { voiceCallHere = target },
      // The person's connectors are the call's too; one that would change something asks in the strip (0.742).
      connectors: true,
      // A drive's throwaway server, in a development build only: LOCUST_VOICE_TEST_SERVER is the path of its script.
      ...(!app.isPackaged && process.env.LOCUST_VOICE_TEST_SERVER !== undefined
        ? { testServer: { command: process.execPath, args: [process.env.LOCUST_VOICE_TEST_SERVER], env: { ELECTRON_RUN_AS_NODE: '1' } } }
        : {}),
      addOn: {
        open: async () => voiceCallAddOn?.open(),
        close: async () => { await voiceCallAddOn?.close() }
      },
      codex: async () => {
        const found = (await discoverForStart('codex')).find((entry) => entry.id === 'codex')
        return found?.readiness === 'ready' ? found.executable : undefined
      },
      spawn: (executablePath, args, env) => spawnAppServer(executablePath, args, env),
      folder: () => (workspaceChosen ? workspacePath : undefined),
      send: (update) => {
        const deliver = (said: typeof update): void => {
          if (voiceCallWindow !== undefined && !voiceCallWindow.isDestroyed()) voiceCallWindow.send(VOICE_CALL_EVENT, said)
        }
        if (update.kind !== 'handed') { deliver(update); return }
        // The voice names a teammate by its id (it read list_teammates): the strip says the name (the drive's
        // first live hand-off read "Handed to tm_wren").
        void listTeammates().then((list) => {
          const named = list.ok ? list.data.teammates.find((teammate) => teammate.teammateId === update.to || teammate.name.toLowerCase() === update.to.toLowerCase()) : undefined
          deliver(named === undefined ? update : { ...update, to: named.name })
        }).catch(() => deliver(update))
      },
      version: app.getVersion()
    })
    const voiceCallRefused = { ok: false, message: 'A voice call could not start here.' } as const
    ipcMain.handle(VOICE_CALL_AVAILABLE, async (event) => {
      if (!fromVoiceWindow(event) || !talkRunsOn(process.platform)) return { available: false, why: 'Voice calls are on Windows and Mac for now.' }
      const answer = await voiceCalls.available()
      // Talk is ready: its keys may be taken now, if they were waiting for it (0.755).
      if (answer.available) void talkShortcut?.ensure()
      return answer
    })
    const callWindow = (event: Electron.IpcMainInvokeEvent): void => {
      if (voiceCallWindow === event.sender) return
      voiceCallWindow = event.sender
      // A window that goes ends its call (once per window, not once per call) -- and so does one closed to
      // the tray, which is hidden rather than destroyed: the microphone and the voice went on behind it
      // (Sol's review of 0.737, #7).
      event.sender.once('destroyed', () => { void voiceCalls.stop() })
      BrowserWindow.fromWebContents(event.sender)?.on('hide', () => { void voiceCalls.stop() })
    }
    ipcMain.handle(VOICE_CALL_START, (event, offer: unknown, target: unknown) => {
      if (!fromVoiceWindow(event) || !talkRunsOn(process.platform)) return voiceCallRefused
      if (!isVoiceCallOffer(offer)) return voiceCallRefused
      callWindow(event)
      return voiceCalls.start(offer, isVoiceCallTarget(target) ? target : undefined)
    })
    /*
     * HEARING A VOICE BEFORE CHOOSING IT (0.751): the first play is a short call with that voice alone, answering a
     * spoken "Hello?" the computer's own speech makes once; the greeting the window records is kept, and plays after that.
     */
    const voicePreviews = createVoicePreviews({
      root: join(app.getPath('userData'), 'voice-previews'),
      speak: (wavPath) => new Promise<void>((resolve, reject) => {
        const { command, args } = speakHello(process.platform, wavPath)
        execFile(command, [...args], { windowsHide: true, timeout: 20_000 }, (error) => { if (error) reject(error); else resolve() })
      })
    })
    ipcMain.handle(VOICE_CALL_PREVIEW, (event, offer: unknown, voice: unknown) => {
      if (!fromVoiceWindow(event) || !talkRunsOn(process.platform)) return voiceCallRefused
      if (!isVoiceCallOffer(offer) || !isChatGptVoice(voice)) return voiceCallRefused
      callWindow(event)
      return voiceCalls.start(offer, undefined, voice)
    })
    ipcMain.handle(VOICE_PREVIEW_SAVED, (event, voice: unknown) => fromOwnWindow(event) ? voicePreviews.saved(voice) : undefined)
    ipcMain.handle(VOICE_PREVIEW_SAVE, (event, voice: unknown, audio: unknown) => fromOwnWindow(event) ? voicePreviews.save(voice, audio) : false)
    ipcMain.handle(VOICE_PREVIEW_TRIGGER, (event) => fromVoiceWindow(event) && talkRunsOn(process.platform) ? voicePreviews.trigger() : undefined)
    ipcMain.handle(VOICE_CALL_STOP, (event) => fromVoiceWindow(event) ? voiceCalls.stop() : undefined)
    // What was said on each call with a teammate, kept with its conversation (0.747).
    const callLog = createCallLog(join(app.getPath('userData'), 'calls.json'))
    ipcMain.handle(CALLS_SAVE, (event, call: unknown) => fromOwnWindow(event) ? callLog.save(call) : false)
    ipcMain.handle(CALLS_LIST, (event, missionIds: unknown) => fromOwnWindow(event) ? callLog.list(missionIds) : [])
    ipcMain.handle(VOICE_CALL_VOICES, (event) => fromOwnWindow(event) ? voiceCallVoice() : DEFAULT_CHATGPT_VOICE)
    ipcMain.handle(VOICE_CALL_SET_VOICE, async (event, voice: unknown) => {
      if (!fromOwnWindow(event) || !isChatGptVoice(voice)) return voiceCallVoice()
      await keepTalkSetting({ voice })
      return voiceCallVoice()
    })
    ipcMain.handle(VOICE_CALL_ANSWER, (event, id: unknown, allowed: unknown) => { if (fromVoiceWindow(event)) voiceCalls.answer(id, allowed) })
    app.once('will-quit', () => voiceCalls.dispose())

    /*
     * TALK FROM ANY APP (0.755, talk-shortcut.ts): the keys bring the window up and press its Talk button. The press
     * is the person's own, so the window is told it was one: a call's voice plays with no click in the window first
     * (Chromium holds back sound a page starts on its own). A press made while there was no window to hear it -- Locust
     * started in the tray -- waits a few seconds for the window it opens.
     */
    const driven = process.argv.some((argument) => argument.startsWith('--remote-debugging-port'))
    const keysHere = process.env.LOCUST_TALK_SHORTCUT
    let talkPressWaiting: { readonly press: TalkShortcutPress; readonly at: number } | undefined
    const talkPressed = async (): Promise<void> => {
      const ready = await voiceCalls.available().catch(() => ({ available: false as const, why: 'Talk could not be checked. Try again.' }))
      const press: TalkShortcutPress = ready.available ? {} : { why: ready.why }
      const window = approvalWindow !== undefined && !approvalWindow.isDestroyed() ? approvalWindow : undefined
      if (window === undefined || window.webContents.isLoading()) {
        talkPressWaiting = { press, at: Date.now() }
        if (window === undefined) openFromBackground?.()
        return
      }
      if (window.isMinimized()) window.restore()
      if (!window.isVisible()) window.show()
      window.focus()
      await window.webContents.executeJavaScript('0', true).catch(() => undefined)
      window.webContents.send(TALK_SHORTCUT_PRESSED, press)
    }
    const shortcut = createTalkShortcut({
      register: (keys, onPress) => globalShortcut.register(keys, onPress),
      unregister: (keys) => { globalShortcut.unregister(keys) },
      read: async () => (await talkSettings()).shortcut,
      write: (keys) => keepTalkSetting({ shortcut: keys }),
      pressed: () => { void talkPressed() },
      ...(process.platform !== 'win32'
        ? { refusedHere: 'Talk from any app is on Windows for now.' }
        : driven
          ? { refusedHere: 'A window a test drive opened leaves your keys alone.', ...(isTalkShortcut(keysHere) ? { keysHere } : {}) }
          : {}),
      ready: async () => (await voiceCalls.available()).available
    })
    talkShortcut = shortcut
    ipcMain.handle(TALK_SHORTCUT_READ, (event) => fromOwnWindow(event) ? shortcut.read() : { working: false })
    ipcMain.handle(TALK_SHORTCUT_SET, async (event, keys: unknown) => {
      if (!fromOwnWindow(event)) return { working: false }
      return shortcut.set(keys).catch(() => shortcut.read())
    })
    ipcMain.handle(TALK_SHORTCUT_PENDING, async (event) => {
      const waiting = talkPressWaiting
      talkPressWaiting = undefined
      if (!fromOwnWindow(event) || waiting === undefined || Date.now() - waiting.at > 15_000) return undefined
      await event.sender.executeJavaScript('0', true).catch(() => undefined)
      return waiting.press
    })
    // A link's message, taken once by the window that shows it (0.770, locust-links.ts).
    ipcMain.handle(LOCUST_LINK_TAKE, (event) => (fromOwnWindow(event) ? linkInbox.take() : undefined))
    // At launch, once Talk has had time to be found; then each time the window finds it ready (VOICE_CALL_AVAILABLE).
    setTimeout(() => { void shortcut.ensure() }, 4_000)
    app.once('will-quit', () => shortcut.dispose())

    const teammatesUnavailable = {
      ok: false,
      error: { code: 'TEAMMATES_UNAVAILABLE', message: 'The local teammate roster could not be read.' }
    } as const

    const teammateRejected = (message: string) =>
      ({ ok: false, error: { code: 'TEAMMATE_REJECTED', message } }) as const

    /*
     * A3.3: the check command is kept PER FOLDER (the settings are the app's),
     * and the window only ever sees and sets this folder's. No folder, no
     * command: a check runs in a project, and there is none open.
     */
    const forThisFolder = (settings: WorkspaceSettings): WorkspaceSettings => {
      const checkFolderId = workspaceChosen ? workspaceIdFor(workspacePath) : undefined
      const { checkCommands, checkFixes, ...rest } = settings
      const command = checkFolderId === undefined ? undefined : checkCommands?.[checkFolderId]
      // And what this folder does when its check fails (0.812): the most fixes, or none.
      const fixes = checkFolderId === undefined ? undefined : parsedCheckFixTries(checkFixes?.[checkFolderId])
      return { ...rest, ...(command === undefined ? {} : { checkCommand: command }), ...(fixes === undefined ? {} : { checkFixTries: fixes }) }
    }
    const withThisFolder = async (requested: unknown): Promise<unknown> => {
      if (typeof requested !== 'object' || requested === null || (!('checkCommand' in requested) && !('checkFixTries' in requested))) return requested
      const { checkCommand, checkFixTries, ...rest } = requested as Record<string, unknown>
      const checkFolderId = workspaceChosen ? workspaceIdFor(workspacePath) : undefined
      if (checkFolderId === undefined) return rest
      const held = await teammates.readSettings()
      const commands = { ...(held.checkCommands ?? {}) }
      if ('checkCommand' in requested) {
        const command = parsedCheckCommand(checkCommand)
        if (command === undefined) delete commands[checkFolderId]
        else commands[checkFolderId] = command
      }
      // 0 or anything that is not 1 to 5 turns it off for this folder: show the failure, as before.
      const fixes = { ...(held.checkFixes ?? {}) }
      if ('checkFixTries' in requested) {
        const most = parsedCheckFixTries(checkFixTries)
        if (most === undefined) delete fixes[checkFolderId]
        else fixes[checkFolderId] = most
      }
      return { ...rest, checkCommands: commands, checkFixes: fixes }
    }
    ipcMain.handle(WORKSPACE_SETTINGS_READ_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { swarm: false, relay: true, relayHopCap: DEFAULT_RELAY_HOP_CAP, memoryMode: DEFAULT_MEMORY_MODE, autoMode: false, askConnectors: false, keepATodoList: false, layout: 'auto', tube: 'full', replySize: 'standard' } as const
      try {
        return forThisFolder(await teammates.readSettings())
      } catch {
        // An unreadable switch reads as its default: swarm off, replies on.
        return { swarm: false, relay: true, relayHopCap: DEFAULT_RELAY_HOP_CAP, memoryMode: DEFAULT_MEMORY_MODE, autoMode: false, askConnectors: false, keepATodoList: false, layout: 'auto', tube: 'full', replySize: 'standard' } as const
      }
    })

    /*
     * The folder ONE teammate works in.
     *
     * Deliberately not the folder switch above. That one reopens Locust,
     * because the ledger, the memory store and the worktrees are all scoped
     * by the project folder; this one moves nothing but where a single
     * teammate's next run stands, so nothing has to be rebound and nothing
     * closes. Colin, 2026-09-09: "it should only change the folder for that
     * chat/teammate not the entire app."
     *
     * The renderer names no path here either. It asks for a teammate; the
     * HOST opens the dialog, checks what came back, and writes it. Clearing
     * needs no dialog and takes the same route so there is one place that
     * decides what a teammate's folder may be.
     */
    /*
     * Every connector the person's Claude Code reports, for the teammate
     * dialog to offer. Refreshed on the way, because a person opening this
     * list has usually just connected something.
     */
    ipcMain.handle(CONNECTOR_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, error: { code: 'CONNECTORS_UNAVAILABLE', message: 'That request was rejected.' } } as const
      try {
        return { ok: true, data: { connectors: await connectorReader.refresh() } } as const
      } catch {
        return { ok: true, data: { connectors: connectorReader.current() } } as const
      }
    })

    /*
     * Narrow one teammate to some of them, or widen it back. The names come
     * from the list above, but the store reads them strictly either way: a
     * name is an allow rule on a real command line.
     */
    // Own browser sign-ins (0.839): taken away, they are cleared, so a teammate turned back on starts signed out.
    ipcMain.handle(TEAMMATE_OWN_BROWSER_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'REJECTED', message: 'The browser sign-ins request was rejected.' } } as const
      }
      const ask = typeof requested === 'object' && requested !== null ? (requested as Record<string, unknown>) : {}
      try {
        const teammate = await teammates.setOwnBrowser(ask.teammateId, ask.on)
        if (teammate.ownBrowser !== true) await browserHost.forgetProfile(teammate.teammateId)
        return { ok: true, data: { teammate } } as const
      } catch (error) {
        return {
          ok: false,
          error: { code: 'REJECTED', message: error instanceof Error ? error.message : 'That could not be changed.' }
        } as const
      }
    })
    ipcMain.handle(TEAMMATE_CONNECTORS_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'REJECTED', message: 'The connector request was rejected.' } } as const
      }
      const ask = typeof requested === 'object' && requested !== null ? (requested as Record<string, unknown>) : {}
      try {
        return { ok: true, data: { teammate: await teammates.setConnectors(ask.teammateId, ask.names) } } as const
      } catch (error) {
        return {
          ok: false,
          error: { code: 'REJECTED', message: error instanceof Error ? error.message : 'That could not be changed.' }
        } as const
      }
    })

    ipcMain.handle(TEAMMATE_FOLDER_CHANNEL, async (event, requested: unknown) => {
      const owner = BrowserWindow.fromWebContents(event.sender)
      if (owner === null || !fromOwnWindow(event)) {
        return { ok: false, error: { code: 'REJECTED', message: 'The folder request was rejected.' } } as const
      }
      const ask = typeof requested === 'object' && requested !== null ? (requested as Record<string, unknown>) : {}
      const teammateId = typeof ask.teammateId === 'string' ? ask.teammateId : ''
      if (ask.clear === true) {
        try {
          return { ok: true, data: { teammate: await teammates.setFolder(teammateId, undefined) } } as const
        } catch (error) {
          return {
            ok: false,
            error: { code: 'REJECTED', message: error instanceof Error ? error.message : 'That could not be changed.' }
          } as const
        }
      }
      let known
      try {
        known = (await teammates.list()).find((teammate) => teammate.teammateId === teammateId)
      } catch {
        known = undefined
      }
      if (known === undefined) {
        return { ok: false, error: { code: 'REJECTED', message: 'That teammate is not on the roster.' } } as const
      }
      const picked = await dialog.showOpenDialog(owner, {
        title: `Choose the folder ${known.name} works in`,
        buttonLabel: 'Work here',
        properties: ['openDirectory', 'createDirectory'],
        defaultPath: known.folder ?? (workspaceChosen ? workspacePath : app.getPath('home'))
      })
      const next = picked.filePaths[0]
      if (picked.canceled || next === undefined) {
        return { ok: false, error: { code: 'CANCELLED', message: 'No folder chosen.' } } as const
      }
      // The same refusal the project folder gets: a teammate pointed at
      // Locust's own installation would be editing the app it is running in,
      // and an update would take its work with it.
      if (isInsideDirectory(next, installDirectory, process.platform)) {
        return {
          ok: false,
          error: { code: 'REJECTED', message: 'That is where Locust itself is installed. Pick a project folder instead.' }
        } as const
      }
      try {
        return { ok: true, data: { teammate: await teammates.setFolder(teammateId, next) } } as const
      } catch (error) {
        return {
          ok: false,
          error: { code: 'REJECTED', message: error instanceof Error ? error.message : 'That folder could not be saved.' }
        } as const
      }
    })

    /*
     * A conversation, in its runtime's own terminal (0.387; open-in-terminal.ts).
     *
     * The window names a mission and nothing else. The session comes from
     * that mission's own record, the folder from its teammate the way a run
     * finds it (peerContextFor), and the program from discovery -- so the
     * only words that reach the terminal are the runtime's own resume flag
     * and a session id checked to be a plain token.
     */
    // The window asks when it shows a conversation, and when it comes back
    // into focus on one (0.391): whatever was done in the terminal meanwhile.
    /*
     * THE TEAM AS A PICTURE OF ITSELF (0.398; shared/team-card.ts).
     *
     * Saving photographs the card the window drew -- this window, never the
     * screen -- and writes the team into the image as data built HERE from the
     * roster, not from anything the window sent. Adding reads that data back
     * and makes each teammate through the store's own checks, as the New
     * teammate dialog does. `LOCUST_TEAM_CARD_PATH` is a drive's stand-in for
     * both file dialogs, which a drive cannot press.
     */
    ipcMain.handle(TEAM_CARD_SAVE_CHANNEL, async (event, raw: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const rect = (typeof raw === 'object' && raw !== null ? raw : {}) as Record<string, unknown>
      const edge = (value: unknown): number | undefined => (typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= 8000 ? Math.round(value) : undefined)
      const [x, y, width, height] = [edge(rect.x), edge(rect.y), edge(rect.width), edge(rect.height)]
      if (x === undefined || y === undefined || width === undefined || height === undefined || width < 40 || height < 40) {
        return { ok: false, message: 'The card could not be photographed.' } as const
      }
      const window = BrowserWindow.fromWebContents(event.sender)
      if (window === null) return { ok: false, message: 'That request was rejected.' } as const
      try {
        const photo = await event.sender.capturePage({ x, y, width, height })
        const team = await teammates.list()
        // The team's routines ride along (0.802), as routine files; a store that will not read leaves them behind, not the card.
        const carried = teamCardRoutines(team, await routines.list().catch(() => []), { workspaceId: workspaceChosen ? memoryWorkspaceId : undefined, connectorNames: connectorReader.names() })
        const card = { ...teamCardOf(team), ...(carried.length === 0 ? {} : { routines: carried }) }
        const png = withTextChunk(photo.toPNG(), TEAM_CARD_KEYWORD, JSON.stringify(card))
        const scripted = process.env.LOCUST_TEAM_CARD_PATH
        const target = scripted !== undefined && scripted.length > 0
          ? scripted
          : (await dialog.showSaveDialog(window, {
              title: 'Save your team as an image',
              defaultPath: join(app.getPath('downloads'), 'Locust team.png'),
              filters: [{ name: 'PNG image', extensions: ['png'] }],
              properties: ['createDirectory', 'showOverwriteConfirmation']
            })).filePath
        if (target === undefined || target.length === 0) return { ok: true } as const
        await writeFile(target, png)
        return { ok: true, path: target } as const
      } catch {
        return { ok: false, message: 'The team card could not be saved. Nothing was written.' } as const
      }
    })

    /*
     * SAVE THE RECORD (0.575; mission-export.ts). A conversation as one
     * Markdown file, from what the ledger holds and nothing else.
     *
     * Only Locust's own window may ask, and the DESTINATION is never the
     * window's: it comes back from a native save dialog, so the only place
     * this writes is one the person just chose by hand. The request names a
     * turn and a tick; the text is built here, from the ledger, never from
     * anything the window sent. Nothing is uploaded and nothing is opened --
     * the file is the person's to read before they send it anywhere.
     * `LOCUST_RECORD_PATH` is a drive's stand-in for the dialog.
     */
    ipcMain.handle(MISSION_RECORD_SAVE_CHANNEL, async (event, raw: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const request = (typeof raw === 'object' && raw !== null ? raw : {}) as Record<string, unknown>
      const missionId = request.missionId
      if (typeof missionId !== 'string' || missionId.length === 0 || missionId.length > 128) return { ok: false, message: 'There is no conversation to save.' } as const
      const window = BrowserWindow.fromWebContents(event.sender)
      if (window === null) return { ok: false, message: 'That request was rejected.' } as const
      try {
        const chain = await recordTurns((id) => missionLedger.getMission(id), missionId)
        const first = chain.missions[0]
        if (first === undefined) return { ok: false, message: 'That conversation is not in the ledger, so there is nothing to save.' } as const
        const owners = await teammates.missionOwners().catch(() => ({}) as Record<string, string>)
        // The newest turn that has an owner on file: a turn handed over by Locust may not.
        const ownerId = [...chain.missions].reverse().map((turn) => owners[turn.metadata.missionId]).find((id) => id !== undefined)
        const owner = ownerId === undefined ? undefined : (await teammates.list().catch(() => [])).find((entry) => entry.teammateId === ownerId)
        // Where the teammate works NOW: its own folder, else the project's, and only for a record from this project.
        const folder = owner?.folder ?? (workspaceChosen && first.metadata.workspaceId === memoryWorkspaceId ? workspacePath : undefined)
        const now = new Date()
        const savedAt = now.toISOString()
        const markdown = missionRecordMarkdown({
          missions: chain.missions,
          ...(owner === undefined ? {} : { teammate: owner.name }),
          ...(folder === undefined ? {} : { folder }),
          locustVersion: app.getVersion(),
          savedAt,
          ...(chain.missingParent === undefined ? {} : { missingParent: chain.missingParent })
        })
        const scripted = !app.isPackaged ? process.env.LOCUST_RECORD_PATH : undefined
        const target = scripted !== undefined && scripted.length > 0
          ? scripted
          : (await dialog.showSaveDialog(window, {
              title: 'Save the record',
              defaultPath: join(app.getPath('downloads'), recordFileName(owner?.name, now)),
              filters: [{ name: 'Markdown', extensions: ['md'] }],
              properties: ['createDirectory', 'showOverwriteConfirmation']
            })).filePath
        // Cancelled is not a failure: nothing was written.
        if (target === undefined || target.length === 0) return { ok: true } as const
        await writeFile(target, markdown, 'utf8')
        if (request.includeRaw !== true) return { ok: true, path: target } as const
        // Beside the file, under its name; never over a file that is already there.
        const stem = target.replace(/\.[^./\\]+$/, '')
        let rawPath = `${stem}.json`
        for (let copy = 2; copy < 100; copy += 1) {
          if (await stat(rawPath).then(() => false, () => true)) break
          rawPath = `${stem} (${String(copy)}).json`
        }
        await writeFile(rawPath, rawRecordJson(chain.missions, app.getVersion(), savedAt), { encoding: 'utf8', flag: 'wx' })
        return { ok: true, path: target, rawPath } as const
      } catch {
        return { ok: false, message: 'The record could not be saved. Nothing was sent anywhere.' } as const
      }
    })

    ipcMain.handle(TEAM_CARD_ADD_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const window = BrowserWindow.fromWebContents(event.sender)
      if (window === null) return { ok: false, message: 'That request was rejected.' } as const
      const scripted = process.env.LOCUST_TEAM_CARD_PATH
      const picked = scripted !== undefined && scripted.length > 0
        ? scripted
        : (await dialog.showOpenDialog(window, {
            title: 'Add a team from an image',
            buttonLabel: 'Add team',
            filters: [{ name: 'Locust team card', extensions: ['png'] }],
            properties: ['openFile']
          })).filePaths[0]
      if (picked === undefined) return { ok: true, added: [], skipped: [], instructed: [], routines: [], routinesLeft: [] } as const
      try {
        if ((await stat(picked)).size > 20 * 1024 * 1024) return { ok: false, message: 'That image is too large to be a team card.' } as const
        const text = readTextChunk(await readFile(picked), TEAM_CARD_KEYWORD)
        const parsed: unknown = text === undefined ? undefined : JSON.parse(text)
        const entries = parsed === undefined ? undefined : readTeamCard(parsed)
        if (entries === undefined) return { ok: false, message: 'That image is not a Locust team card: it carries no team.' } as const
        const taken = new Set((await teammates.list()).map((teammate) => teammate.name))
        const added: string[] = []
        const skipped: string[] = []
        const instructed: string[] = []
        // Who each place on the card became here, for the routines that name them by place.
        const made: ({ readonly teammateId: string; readonly name: string; readonly route: TeammateRoute | undefined } | undefined)[] = []
        for (const entry of entries) {
          const given = typeof entry.name === 'string' ? entry.name.trim() : ''
          try {
            const name = freeName(given, taken)
            const teammate = await teammates.create({ name, hue: entry.hue, role: entry.role, roleTitle: entry.roleTitle, avatar: entry.avatar, instructions: entry.instructions, starters: entry.starters })
            // A route this folder's policy refuses is left off: the teammate comes, and picks a model it may use (0.802).
            const route = isTeammateRoute(entry.route) && policyTurnRefusal(organizationPolicy, entry.route) === undefined ? entry.route : undefined
            if (route !== undefined) await teammates.rememberRoute(teammate.teammateId, route)
            taken.add(name)
            added.push(name)
            if (typeof entry.instructions === 'string' && entry.instructions.trim().length > 0) instructed.push(name)
            made.push({ teammateId: teammate.teammateId, name, route })
          } catch {
            skipped.push(given.length > 0 ? given : 'A teammate with no name')
            made.push(undefined)
          }
        }
        /*
         * THE ROUTINES, PAUSED (0.802). Each is read as an imported routine
         * file is, given to the teammate it belonged to, and made as Import
         * makes one: on no schedule and read-only until its owner changes
         * that, in this folder. One that cannot come is named, with why.
         */
        const brought: string[] = []
        const left: string[] = []
        for (const entry of readTeamCardRoutines(parsed, entries.length)) {
          const file = cardRoutineFile(entry)
          const owner = made[entry.teammate]
          const name = file?.name ?? 'A routine'
          if (file === undefined || owner === undefined) { left.push(`${name} (it did not read)`); continue }
          if (!workspaceChosen) { left.push(`${name} (open a project folder for it first)`); continue }
          if (owner.route === undefined) { left.push(`${name} (${owner.name} has no model yet)`); continue }
          try {
            await routines.create({ name: file.name, steps: file.steps, inputs: file.inputs, teammateId: owner.teammateId, route: { ...owner.route, mode: 'ask' }, learnedFrom: [], workspaceId: memoryWorkspaceId,
              // A step that waited for an OK where the card was made still waits here (0.840).
              ...(file.handOffs.some((entry) => entry.askFirst === true) ? { handOffs: file.handOffs.map((entry) => (entry.askFirst === true ? { askFirst: true as const } : {})) } : {}) })
            brought.push(file.name)
          } catch {
            left.push(`${name} (it could not be saved)`)
          }
        }
        return { ok: true, added, skipped, instructed, routines: brought, routinesLeft: left } as const
      } catch {
        return { ok: false, message: 'That image could not be read as a Locust team card.' } as const
      }
    })

    ipcMain.handle(TERMINAL_CATCH_UP_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event) || typeof requested !== 'string' || requested.length === 0 || requested.length > 128) {
        return { imported: 0 }
      }
      const result = await catchUp(requested).catch(() => ({ imported: 0, latestMissionId: requested }))
      return result
    })

    /*
     * IMPORT A CONVERSATION (session-import.ts). The list is the host's, and
     * an import names a session from it by runtime and id: the folder and the
     * title are taken from the file, never from the window.
     */
    let lastListing: readonly ImportableSession[] = []
    const cursorSessionHome = join(homedir(), '.cursor')
    const openCodeSessions = createOpenCodeSessionReader(executableLocator, sessionProbeRunner)
    ipcMain.handle(SESSION_IMPORT_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      try {
        // Sessions Locust already holds: every one read through before.
        const held = await readFile(terminalImportsFile, 'utf8').then((text) => Object.keys(JSON.parse(text) as Record<string, unknown>), () => [] as string[])
        const warnings: string[] = []
        const otherSessions = [
          ...await openCodeSessions.list().catch(() => { warnings.push('OpenCode sessions could not be listed. Your other sessions are still shown.'); return [] }),
          ...await listCursorSessions(cursorSessionHome).catch(() => { warnings.push('Cursor Agent sessions could not be listed. Your other sessions are still shown.'); return [] })
        ]
        // These CLIs do not label a session as Locust-owned. Exclude the
        // thread IDs already recorded here before offering outside chats.
        if (otherSessions.length > 0) {
          for (const mission of (await missionLedger.listMissions()).missions) {
            const id = runtimeThreadIdOf(mission)
            if (id !== undefined) held.push(id)
          }
        }
        lastListing = await listImportableSessions(sessionPlaces, { skip: new Set(held), otherSessions })
        return {
          ok: true,
          warnings,
          sessions: lastListing.map((session) => ({ ...session, folderName: basename(session.cwd) || session.cwd }))
        } as const
      } catch {
        return { ok: false, message: 'The sessions could not be listed. Nothing was changed; close this and open it again.' } as const
      }
    })
    ipcMain.handle(SESSION_IMPORT_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const asked = (typeof requested === 'object' && requested !== null ? requested : {}) as Record<string, unknown>
      const session = lastListing.find((one) => one.runtime === asked.runtime && one.sessionId === asked.sessionId)
      if (session === undefined) return { ok: false, message: 'That session is not in the list any more. Open the list again.' } as const
      const result = await importSession(session, {
        ledger: missionLedger,
        imports: terminalImports,
        workspaceIdFor,
        sameFolder: (cwd) => folders.sameAs(cwd),
        learnFolder: (id, path) => folders.learn([{ id, path }]),
        nameConversation: (missionId, title) => teammates.renameMission(missionId, title),
        readExchanges: (session) => session.runtime === 'cursor' ? cursorExchanges(cursorSessionHome, session) : openCodeSessions.exchanges(session.sessionId),
        pathOf: (runtime, sessionId) => transcriptPathFor(runtime, sessionId, sessionPlaces)
      }).catch((error: unknown) => ({ ok: false, message: error instanceof Error ? `It could not be brought in: ${error.message}` : 'It could not be brought in.' }) as const)
      if (result.ok) {
        lastListing = lastListing.filter((one) => one !== session)
        note('session-imported', `${session.runtime} ${String(result.turns)} turns`)
      }
      return result
    })

    const terminalHost = createTerminalHost({
      refusal: () => policyTerminalRefusal(organizationPolicy),
      folder: context => terminalFolder(context, {
        current: () => workspaceChosen ? workspacePath : undefined,
        mission: async id => (await missionLedger.getMission(id))?.metadata,
        folderOf: workableFolderOf,
        ownerOf: async id => (await teammates.missionOwners())[id],
        teammates: () => teammates.list()
      }),
      emit: event => { if (approvalWindow !== undefined && !approvalWindow.isDestroyed()) approvalWindow.webContents.send(TERMINAL_EVENT, event) }
    })
    terminalForShutdown = terminalHost
    registerTerminalIpc(terminalHost, fromOwnWindow)
    const ownCommands = createOwnCommandHost({
      root: app.getPath('userData'),
      refusal: () => policyTerminalRefusal(organizationPolicy),
      shells: () => terminalShells(),
      folder: context => terminalFolder(context, {
        current: () => workspaceChosen ? workspacePath : undefined,
        mission: async id => (await missionLedger.getMission(id))?.metadata,
        folderOf: workableFolderOf,
        ownerOf: async id => (await teammates.missionOwners())[id],
        teammates: () => teammates.list()
      })
    })
    ownCommandsForShutdown = ownCommands
    registerOwnCommandIpc(ownCommands, fromOwnWindow)
    registerBrowserIpc(browserHost, fromOwnWindow)

    ipcMain.handle(OPEN_IN_TERMINAL_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event) || typeof requested !== 'string' || requested.length === 0 || requested.length > 128) {
        return { ok: false, message: 'That conversation cannot be opened in a terminal.' } as const
      }
      const request = await terminalRequestFor(requested, {
        liveMissionIds: () => [...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()],
        getMission: async (missionId) => {
          const mission = await missionLedger.getMission(missionId)
          // Antigravity's own walk: never a conversation its app held, which the CLI does not have (0.543).
          return mission === undefined ? undefined : { runtime: mission.metadata.runtime, model: mission.metadata.model, ...(accountFromRoute(mission.metadata.resolvedRouteId) === undefined ? {} : { account: accountFromRoute(mission.metadata.resolvedRouteId) }), session: mission.metadata.runtime === 'antigravity' ? resumableThreadOf(mission, 'antigravity') : runtimeThreadIdOf(mission) }
        },
        ownerOf: async (missionId) => {
          const [owners, roster] = await Promise.all([teammates.missionOwners(), teammates.list()])
          const owner = roster.find((entry) => entry.teammateId === owners[missionId])
          return owner === undefined ? undefined : { teammateId: owner.teammateId, name: owner.name }
        },
        cwdFor: async (teammateId) => (await peerContextFor(teammateId))?.cwd,
        workspacePath,
        launchFor: async (runtime, account) => (await (account === undefined ? discoverForWork() : discoverForAccount(runtime, account))).find((entry) => entry.id === runtime)?.executable
      })
      if ('refused' in request) return { ok: false, message: request.refused } as const
      if (!isMissionRuntime(request.runtime)) return { ok: false, message: 'That AI agent is not supported here.' } as const
      const managed = policyUnmeteredRefusal(organizationPolicy, { runtime: request.runtime, mode: 'auto', account: request.account })
      if (managed !== undefined) return { ok: false, message: managed } as const
      return openInTerminal(request, {
        ...(process.env.LOCALAPPDATA === undefined ? {} : { localAppData: process.env.LOCALAPPDATA }),
        // The drives' seam: a console window is one they can find and close.
        ...(process.env.LOCUST_TERMINAL === 'console' ? { prefer: 'console' as const } : {})
      })
    })

    // Start here, on the window's question about a folder like home or .claude (0.790): the CURRENT folder is kept as
    // said yes to. The window cannot name another, and a project folder has nothing to say yes to.
    ipcMain.handle(WORKSPACE_CONFIRM_CHANNEL, async (event) => {
      if (!fromOwnWindow(event) || !workspaceChosen || folderCaution(workspacePath) === undefined) return false
      await confirmFolder(workspacePath)
      return true
    })

    ipcMain.handle(WORKSPACE_CHOOSE_CHANNEL, async (event) => {
      const owner = BrowserWindow.fromWebContents(event.sender)
      if (owner === null || !fromOwnWindow(event)) {
        return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The folder request was rejected.' } } as const
      }
      const picked = await dialog.showOpenDialog(owner, {
        title: 'Choose the folder your teammates work in',
        buttonLabel: 'Work here',
        properties: ['openDirectory', 'createDirectory'],
        defaultPath: workspaceChosen ? workspacePath : app.getPath('home')
      })
      const next = picked.filePaths[0]
      if (picked.canceled || next === undefined) {
        return { ok: false, error: { code: 'CANCELLED', message: 'No folder chosen.' } } as const
      }
      /*
       * A FOLDER LIKE HOME OR .claude, PICKED HERE (0.816): no Windows dialog. It asked in a message box since 0.582;
       * the first new conversation in such a folder has asked in the window since 0.790 -- Choose another folder, or
       * Start here -- and that is the one place it is asked now. Colin, 2026-10-18, of the same question as a Windows
       * dialog: "maybe make it part of the app?"
       */
      if (isInsideDirectory(next, installDirectory, process.platform)) {
        return {
          ok: false,
          error: {
            code: 'INSTALL_FOLDER',
            message: 'That is where Locust itself is installed. Pick a project folder instead.'
          }
        } as const
      }
      // In place, never a restart (0.458): the window's folder simply changes.
      try {
        const now = await switchFolder(next)
        return { ok: true, data: { path: now.path, id: now.id, name: now.name } } as const
      } catch {
        return {
          ok: false,
          error: { code: 'INTERNAL_ERROR', message: 'The chosen folder could not be saved.' }
        } as const
      }
    })

    /*
     * A QUESTION ON THE SIDE (0.461, Devin's side chats): asked of a COPY of
     * the conversation's session -- read-only, on the conversation's own
     * runtime and model, in a run slot of its own so it answers while the
     * conversation keeps going. The conversation never sees it
     * (codex-mission.ts, `side`).
     */
    const askOnTheSide = async (of: unknown, question: unknown, count: unknown, effortOverride?: { readonly effort?: string }) => {
      const refuse = (message: string) => ({ ok: false, message }) as const
      if (typeof of !== 'string' || typeof question !== 'string' || question.trim().length === 0) return refuse('Write the question first.')
      const asked = typeof count === 'number' && Number.isSafeInteger(count) && count >= 1 ? count : 1
      const turn = await missionLedger.getMission(of).catch(() => undefined)
      if (turn === undefined) return refuse('That conversation is not in the record.')
      const owner = (await teammates.missionOwners().catch(() => ({} as Record<string, string>)))[of]
      const peer = owner === undefined ? undefined : await peerContextFor(owner)
      const effort = effortOverride === undefined ? (turn.metadata as { readonly effort?: unknown }).effort : effortOverride.effort
      const inRemovedCopy = (await compares.list().catch(() => [])).some(
        (compare) => compare.changes === true && compare.changesIn !== 'folder' && compare.kept !== undefined && compare.slots.some((slot) => slot.missionIds.includes(of))
      )
      const response = await codexMissions.start(
        question.trim(),
        turn.metadata.runtime,
        'ask',
        { ...(turn.metadata.model === 'account-default' ? {} : { model: turn.metadata.model }), ...(accountFromRoute(turn.metadata.resolvedRouteId) === undefined ? {} : { account: accountFromRoute(turn.metadata.resolvedRouteId) }), ...(typeof effort === 'string' ? { effort } : {}) },
        sendToWindow,
        undefined,
        peer,
        undefined,
        undefined,
        undefined,
        undefined,
        { key: `side:${of}` },
        { of, question: asked, ...(inRemovedCopy ? { freshSession: true } : {}) }
      )
      if (!response.ok) return refuse(response.error.message)
      sendToWindow({
        kind: 'mission-started',
        runId: response.data.runId,
        missionId: response.data.missionId,
        // No owner: the teammate is not busy with it, and its face does not say so.
        prompt: question.trim(),
        data: response.data,
        startedBy: { kind: 'side', of, question: asked }
      })
      return { ok: true, data: { runId: response.data.runId, missionId: response.data.missionId } } as const
    }
    ipcMain.handle(SIDE_ASK_CHANNEL, async (event, of: unknown, question: unknown, count: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      return askOnTheSide(of, question, count)
    })
    // /tune explicitly authorizes this one side turn; the card gates the three trials separately.
    ipcMain.handle(TUNE_DRAFT_CHANNEL, async (event, of: unknown) => {
      const refuse = (message: string) => ({ ok: false, message }) as const
      if (!fromOwnWindow(event) || typeof of !== 'string') return refuse('That request was rejected.')
      const turn = await missionLedger.getMission(of).catch(() => undefined)
      if (turn === undefined || !turn.events.some((entry) => ['run.completed', 'run.failed', 'run.cancelled'].includes(entry.type))) return refuse('Finish a turn in this conversation first. Nothing was started.')
      const owner = (await teammates.missionOwners())[of]
      const teammate = (await teammates.list()).find((entry) => entry.teammateId === owner)
      if (teammate === undefined) return refuse('Open a teammate’s conversation first. Nothing was started.')
      if (teammate.route === undefined || teammate.route.runtime !== turn.metadata.runtime || teammate.route.model !== turn.metadata.model || teammate.route.account !== accountFromRoute(turn.metadata.resolvedRouteId)) return refuse('This conversation uses another model or account. Drafts were not requested; you can write the two alternatives yourself.')
      // "What it knows" is drafted as a skill (0.811) for every agent a run can hand one to (0.813: Codex, OpenCode, Copilot and Antigravity too).
      return askOnTheSide(of, tuneDraftPrompt(teammate.instructions ?? '', await tuneLessons.list(teammate.teammateId), { skills: takesSkills(teammate.route.runtime), runtimeName: runtimeDisplayName(teammate.route.runtime) ?? teammate.route.runtime }), 1, teammate.route)
    })
    // Note what a kept tune showed (0.810): one side turn on the teammate's own route, only when pressed.
    ipcMain.handle(TUNE_LESSON_NOTE_CHANNEL, async (event, compareId: unknown, usedInstructions: unknown, measured: unknown) => {
      const refuse = (message: string) => ({ ok: false, message }) as const
      if (!fromOwnWindow(event) || typeof compareId !== 'string') return refuse('That request was rejected.')
      const compare = await compares.get(compareId).catch(() => undefined)
      if (compare === undefined || compare.kept === undefined || compare.teammateId === undefined || !hasColumnInstructions(compare)) {
        return refuse('Keep a setup in a teammate’s tune first. Nothing was started.')
      }
      const keptColumn = compare.slots.find((column) => column.slot === compare.kept?.slot)
      const of = keptColumn?.missionIds.at(-1)
      const turn = of === undefined ? undefined : await missionLedger.getMission(of).catch(() => undefined)
      if (of === undefined || turn === undefined || !turn.events.some((entry) => ['run.completed', 'run.failed', 'run.cancelled'].includes(entry.type))) {
        return refuse('Wait for the kept run to finish first. Nothing was started.')
      }
      const teammate = (await teammates.list()).find((entry) => entry.teammateId === compare.teammateId)
      if (teammate === undefined) return refuse('That teammate is no longer on the team. Nothing was started.')
      if (teammate.route === undefined || teammate.route.runtime !== turn.metadata.runtime || teammate.route.model !== turn.metadata.model || teammate.route.account !== accountFromRoute(turn.metadata.resolvedRouteId)) {
        return refuse('This teammate’s model or account changed since the tune. Nothing was started.')
      }
      const setups: TuneLessonSetup[] = []
      // A TUNE, MEASURED (0.811): each setup's record over every run, as its benchmark shows it, when it was run again.
      const measuredBySlot = new Map((tuneMeasuredOf(measured) ?? []).map(({ slot, ...record }) => [slot, record] as const))
      for (const column of compare.slots) {
        const newest = column.missionIds.at(-1)
        const recorded = newest === undefined ? undefined : await missionLedger.getMission(newest).catch(() => undefined)
        const tracker = createTranscriptTracker()
        if (recorded !== undefined) tracker.track(recorded.events)
        const setup = setupName(column.route) ?? blindName(column.slot, true)
        setups.push({
          setup,
          instructions: column.route.instructions ?? '',
          ...(column.route.why === undefined ? {} : { why: column.route.why }),
          ...(column.route.skill === undefined ? {} : { skill: column.route.skill }),
          answer: tracker.latestFinal?.trim() ?? '',
          ...(compare.changes === true ? { changes: columnChangesSummary(recorded?.events ?? [], column.slot === compare.kept.slot ? compare.kept.brought : undefined) } : {}),
          ...(measuredBySlot.get(column.slot) === undefined ? {} : { measured: measuredBySlot.get(column.slot)! })
        })
      }
      const keptName = (keptColumn === undefined ? undefined : setupName(keptColumn.route)) ?? blindName(compare.kept.slot, true)
      const used = usedInstructions === true || (keptName !== 'Current' && keptColumn?.route.instructions !== undefined && (teammate.instructions ?? '') === keptColumn.route.instructions)
      const side = await askOnTheSide(
        of,
        tuneLessonPrompt({
          ask: compare.prompt,
          setups,
          kept: keptName,
          usedInstructions: used,
          ...(compare.changes === true ? { changedFiles: true } : {})
        }),
        1,
        teammate.route
      )
      if (!side.ok) return side
      for (let waited = 0; waited < 1_800 && codexMissions.hasMission(side.data.missionId); waited += 1) {
        await new Promise((resolve) => setTimeout(resolve, 100))
      }
      if (codexMissions.hasMission(side.data.missionId)) {
        codexMissions.cancel(side.data.runId)
        return refuse(TUNE_LESSON_FALLBACK)
      }
      const finished = await missionLedger.getMission(side.data.missionId).catch(() => undefined)
      const tracker = createTranscriptTracker()
      if (finished !== undefined) tracker.track(finished.events)
      const completed = finished?.events.some((entry) => entry.type === 'run.completed') === true
      const raw = completed ? (tracker.latestFinal ?? '') : ''
      const parsed = parseTuneLesson(raw)
      if (!parsed.ok) return refuse(parsed.message)
      const lesson = parsed.lesson === null
        ? null
        : await tuneLessons.add({
            teammateId: teammate.teammateId,
            compareId: compare.compareId,
            when: parsed.lesson.when,
            do: parsed.lesson.do,
            because: parsed.lesson.because,
            confidence: parsed.lesson.confidence
          })
      return {
        ok: true,
        data: {
          runId: side.data.runId,
          missionId: side.data.missionId,
          edits: parsed.edits,
          lesson,
          ...(parsed.message === undefined ? {} : { message: parsed.message })
        }
      } as const
    })
    // COMBINE BOTH EDITS (0.811, tune-combine.ts): one side turn on the teammate's own route; nothing is saved by it.
    ipcMain.handle(TUNE_COMBINE_CHANNEL, async (event, compareId: unknown) => {
      const refuse = (message: string) => ({ ok: false, message }) as const
      if (!fromOwnWindow(event) || typeof compareId !== 'string') return refuse('That request was rejected.')
      const compare = await compares.get(compareId).catch(() => undefined)
      const setups = compare === undefined || compare.teammateId === undefined ? undefined : combineSetups(compare)
      if (compare === undefined || setups === undefined) return refuse('Keep one of a tune’s two drafts first: there is nothing to combine. Nothing was started.')
      const keptColumn = compare.slots.find((column) => column.slot === compare.kept?.slot)
      const of = keptColumn?.missionIds.at(-1)
      const turn = of === undefined ? undefined : await missionLedger.getMission(of).catch(() => undefined)
      if (of === undefined || turn === undefined || !turn.events.some((entry) => ['run.completed', 'run.failed', 'run.cancelled'].includes(entry.type))) {
        return refuse('Wait for the kept run to finish first. Nothing was started.')
      }
      const teammate = (await teammates.list()).find((entry) => entry.teammateId === compare.teammateId)
      if (teammate === undefined) return refuse('That teammate is no longer on the team. Nothing was started.')
      if (teammate.route === undefined || teammate.route.runtime !== turn.metadata.runtime || teammate.route.model !== turn.metadata.model || teammate.route.account !== accountFromRoute(turn.metadata.resolvedRouteId)) {
        return refuse('This teammate’s model or account changed since the tune. Nothing was started.')
      }
      const side = await askOnTheSide(of, tuneCombinePrompt({ ask: compare.prompt, setups }), 1, teammate.route)
      if (!side.ok) return side
      for (let waited = 0; waited < 1_800 && codexMissions.hasMission(side.data.missionId); waited += 1) {
        await new Promise((resolve) => setTimeout(resolve, 100))
      }
      if (codexMissions.hasMission(side.data.missionId)) {
        codexMissions.cancel(side.data.runId)
        return refuse(TUNE_COMBINE_FALLBACK)
      }
      const finished = await missionLedger.getMission(side.data.missionId).catch(() => undefined)
      const tracker = createTranscriptTracker()
      if (finished !== undefined) tracker.track(finished.events)
      const completed = finished?.events.some((entry) => entry.type === 'run.completed') === true
      const parsed = parseTuneCombine(completed ? (tracker.latestFinal ?? '') : '', setups)
      if (!parsed.ok) return refuse(parsed.message)
      return { ok: true, data: { runId: side.data.runId, missionId: side.data.missionId, instructions: parsed.instructions, note: parsed.note } } as const
    })
    ipcMain.handle(TUNE_LESSON_LIST_CHANNEL, async (event, teammateId: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      try {
        return { ok: true, data: { lessons: await tuneLessons.list(teammateId) } } as const
      } catch {
        return { ok: false, message: 'Tune lessons could not be read.' } as const
      }
    })
    ipcMain.handle(TUNE_LESSON_REMOVE_CHANNEL, async (event, lessonId: unknown) => {
      if (!fromOwnWindow(event) || typeof lessonId !== 'string') return { ok: false, message: 'That request was rejected.' } as const
      try {
        await tuneLessons.remove(lessonId)
        return { ok: true, data: { lessons: await tuneLessons.list() } } as const
      } catch {
        return { ok: false, message: 'That tune lesson could not be removed.' } as const
      }
    })
    // A TEAMMATE'S OWN SKILLS (0.811, teammate-skills.ts): every teammate's, for Edit teammate and the kept strip.
    ipcMain.handle(TEAMMATE_SKILL_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      try {
        return { ok: true, data: { skills: await teammateSkills.list() } } as const
      } catch (error) {
        return { ok: false, message: error instanceof Error ? error.message : 'The teammate skills could not be read.' } as const
      }
    })
    // Use this skill: the skill is read from the kept tune itself, never taken from the window.
    ipcMain.handle(TEAMMATE_SKILL_USE_CHANNEL, async (event, compareId: unknown, which: unknown) => {
      const refuse = (message: string) => ({ ok: false, message }) as const
      if (!fromOwnWindow(event) || typeof compareId !== 'string' || (which !== 'kept' && which !== 'other')) return refuse('That request was rejected.')
      const compare = await compares.get(compareId).catch(() => undefined)
      const skill = compare === undefined ? undefined : which === 'kept' ? keptSkill(compare) : otherSkill(compare)?.skill
      if (compare?.teammateId === undefined || skill === undefined) return refuse('Keep a setup in a teammate’s tune first. The teammate is unchanged.')
      const teammate = (await teammates.list()).find((entry) => entry.teammateId === compare.teammateId)
      if (teammate === undefined) return refuse('That teammate is no longer on the team. Nothing was changed.')
      if (teammate.route?.runtime !== 'claude') return refuse(`${teammate.name} no longer runs on Claude Code, so a skill would not reach it. The teammate is unchanged.`)
      try {
        await teammateSkills.add(teammate.teammateId, skill, compare.compareId)
        return { ok: true, data: { skills: await teammateSkills.list() } } as const
      } catch (error) {
        return refuse(error instanceof Error ? error.message : 'That skill could not be saved. The teammate is unchanged.')
      }
    })
    // A TEAMMATE'S SETUP (0.811, teammate-setup.ts): what shapes its turns, read fresh; nothing is written or started.
    ipcMain.handle(TEAMMATE_SETUP_CHANNEL, async (event, teammateId: unknown, missionId: unknown) => {
      if (!fromOwnWindow(event) || typeof teammateId !== 'string') return { ok: false, message: 'That request was rejected.' } as const
      try {
        const teammate = (await teammates.list()).find((entry) => entry.teammateId === teammateId)
        if (teammate === undefined) return { ok: false, message: 'That teammate is no longer on the team.' } as const
        // The open conversation, only when it is this teammate's: its group, and what its last turn loaded.
        const owned = typeof missionId === 'string' && (await teammates.missionOwners())[missionId] === teammateId ? missionId : undefined
        const turn = owned === undefined ? undefined : await missionLedger.getMission(owned).catch(() => undefined)
        const listedGroups = owned === undefined ? undefined : await groups.list().catch(() => undefined)
        const group = listedGroups === undefined ? undefined : groupBriefFor(await conversationChain.keysBefore(owned), listedGroups)
        const settings = await teammates.readSettings()
        return {
          ok: true,
          data: await gatherTeammateSetup({
            teammate,
            projectPath: workspacePath,
            ...(group === undefined ? {} : { group: { name: group.name, instructions: group.instructions } }),
            settings: { ...(settings.aboutYou === undefined ? {} : { aboutYou: settings.aboutYou }), memoryMode: settings.memoryMode, ...(settings.claudeOwnSkills === undefined ? {} : { claudeOwnSkills: settings.claudeOwnSkills }) },
            memoryCount: async (folder) => (await memories.briefed(workspaceIdFor(folder))).length,
            ownSkills: async () => (await teammateSkills.list(teammateId)).map((skill) => skill.name),
            lessons: async () => (await tuneLessons.list(teammateId)).length,
            libraryFolder: skillLibrary.skillsFolder,
            homeDirectory: homedir(),
            ...(turn === undefined ? {} : { lastTurn: turn.events })
          })
        } as const
      } catch {
        return { ok: false, message: 'This teammate’s setup could not be read. Nothing was changed.' } as const
      }
    })
    ipcMain.handle(TEAMMATE_SKILL_REMOVE_CHANNEL, async (event, teammateId: unknown, name: unknown) => {
      if (!fromOwnWindow(event) || typeof teammateId !== 'string' || typeof name !== 'string') return { ok: false, message: 'That request was rejected.' } as const
      try {
        await teammateSkills.remove(teammateId, name)
        return { ok: true, data: { skills: await teammateSkills.list() } } as const
      } catch {
        return { ok: false, message: 'That skill could not be removed. The teammate’s skills are unchanged.' } as const
      }
    })

    // Every folder worked in, newest first, and the one the window is in (0.458).
    ipcMain.handle(FOLDER_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { folders: [], currentId: undefined }
      // Never Locust's own install folder: early builds ran conversations there, and no teammate should work in it.
      const listed = (await folders.list()).map((folder) => (isInstallFolder(folder.path) ? { ...folder, installFolder: true as const } : folder))
      return { folders: listed, currentId: workspaceChosen ? workspaceIdFor(workspacePath) : undefined }
    })
    // Switch to a folder already worked in -- by id, never a path the window names (0.458).
    /*
     * PUT BACK WHAT THE LATER REPLIES CHANGED (0.502). The window says which
     * files and what was recorded; the host checks every piece of that, stays
     * inside this folder, and writes nothing until each file's answer is
     * known -- then only the files that come back exactly. A link is left
     * alone: following one could write outside the folder.
     */
    ipcMain.handle(REWIND_PUT_BACK_CHANNEL, async (event, request: unknown) => {
      const leftAlone: { path: string; why: string }[] = []
      if (!fromOwnWindow(event)) return { putBack: [], leftAlone }
      const files = typeof request === 'object' && request !== null ? (request as { files?: unknown }).files : undefined
      if (!Array.isArray(files) || files.length > 500) return { putBack: [], leftAlone }
      const rowOk = (row: unknown): boolean =>
        typeof row === 'object' && row !== null
        && ['context', 'add', 'del'].includes((row as { kind?: unknown }).kind as string)
        && typeof (row as { text?: unknown }).text === 'string'
      const hunkOk = (hunk: unknown): boolean => {
        if (typeof hunk !== 'object' || hunk === null) return false
        const h = hunk as Record<string, unknown>
        return ['oldStart', 'oldCount', 'newStart', 'newCount'].every((field) => Number.isSafeInteger(h[field]) && (h[field] as number) >= 0)
          && Array.isArray(h.rows) && h.rows.length <= 100_000 && h.rows.every(rowOk)
      }
      const changeOk = (change: unknown): change is ReverseChange =>
        typeof change === 'object' && change !== null
        && ['MODIFIED', 'ADDED', 'DELETED', 'RENAMED'].includes((change as { status?: unknown }).status as string)
        && Array.isArray((change as { hunks?: unknown }).hunks) && ((change as { hunks: unknown[] }).hunks).every(hunkOk)
      const folder = resolvePath(workspacePath)
      const writes: { full: string; path: string; next: string | null; was: string | undefined }[] = []
      for (const entry of files as unknown[]) {
        const path = typeof entry === 'object' && entry !== null ? (entry as { path?: unknown }).path : undefined
        if (typeof path !== 'string' || path.length === 0) continue
        const cannot = (entry as { cannot?: unknown }).cannot
        if (typeof cannot === 'string') {
          leftAlone.push({ path, why: cannot })
          continue
        }
        const changes = (entry as { changes?: unknown }).changes
        if (!Array.isArray(changes) || changes.length === 0 || !changes.every(changeOk)) {
          leftAlone.push({ path, why: 'its recorded change could not be read' })
          continue
        }
        const full = resolvePath(folder, path)
        const inside = relative(folder, full)
        if (inside.length === 0 || inside.startsWith('..') || isAbsolute(inside)) {
          leftAlone.push({ path, why: 'it is outside this folder' })
          continue
        }
        const facts = await lstat(full).catch(() => undefined)
        if (facts !== undefined && !facts.isFile()) {
          leftAlone.push({ path, why: 'it is not a plain file' })
          continue
        }
        const content = facts === undefined ? undefined : await readFile(full, 'utf8').catch(() => undefined)
        const result = reverseChanges(content, changes)
        if (!result.ok) {
          leftAlone.push({ path, why: result.why })
          continue
        }
        writes.push({ full, path, next: result.next, was: content })
      }
      /*
       * ALL OF THEM, OR NONE (0.512). Sol's long pass on 0.509: eight files
       * offered, six put back, two left because their change was recorded
       * without the lines -- and the six had removed `src/storage.js` that
       * one of the two still imported, so the project's own tests failed
       * before the new reply had done anything. Each file was exact; the set
       * was not. A project half put back is worse than one not put back.
       */
      if (leftAlone.length > 0) return { putBack: [], leftAlone, heldBack: writes.map((write) => write.path) }
      const done: typeof writes = []
      for (const write of writes) {
        try {
          if (write.next === null) await rm(write.full, { force: true })
          else {
            await mkdir(dirname(write.full), { recursive: true })
            await writeFile(write.full, write.next, 'utf8')
          }
          done.push(write)
        } catch {
          // The ones already written go back to how they were, so the folder is never half put back.
          for (const undo of done.reverse()) {
            try {
              if (undo.was === undefined) await rm(undo.full, { force: true })
              else await writeFile(undo.full, undo.was, 'utf8')
            } catch { /* said below: the folder may not be as it was */ }
          }
          return { putBack: [], leftAlone: [{ path: write.path, why: 'it could not be written' }], heldBack: writes.filter((other) => other !== write).map((other) => other.path) }
        }
      }
      return { putBack: done.map((write) => write.path), leftAlone }
    })

    /*
     * CLOUD TASKS (0.503, main/cloud-tasks.ts). Codex Cloud, reached through
     * the Codex CLI the person already signed in to; the task's folder is the
     * one open now. A drive's window spends nothing, so it never starts one.
     */
    const codexLaunch = async () => {
      const codex = (await discoverForWork().catch(() => [])).find((entry) => entry.id === 'codex')
      return codex?.availability === 'available' && codex.readiness === 'ready' ? codex.executable : undefined
    }
    const gitRunner: Runner = (args, cwd) => new Promise((resolve) => {
      execFile('git', ownGitArgs(args), { cwd, windowsHide: true, maxBuffer: 4 * 1024 * 1024, timeout: 20_000 }, (error, stdout, stderr) => {
        resolve({ code: error === null ? 0 : typeof (error as { code?: unknown }).code === 'number' ? (error as { code: number }).code : 1, stdout: String(stdout), stderr: String(stderr) })
      })
    })
    // For quick read-only questions: whether an environment exists (0.505).
    // Each leaves the folder's error.log as it found it (0.681, leavingNoLog).
    const cloudAsk = leavingNoLog(launchRunner(codexLaunch, 45_000))
    const cloudTasks = createCloudTaskService({
      file: join(app.getPath('userData'), 'cloud-tasks.json'),
      codex: leavingNoLog(launchRunner(codexLaunch, 180_000)),
      git: gitRunner
    })

    /*
     * CLAUDE WORK THAT KEEPS GOING WHEN LOCUST CLOSES (W10, claude-background-runs.ts).
     * Claude Code's own `--bg` sessions: Locust starts one, watches it while it lives,
     * and brings its turn into the conversation when it ends -- also after a relaunch.
     */
    const claudeLaunch = async () => {
      const claude = (await discoverForWork().catch(() => [])).find((entry) => entry.id === 'claude')
      return claude?.availability === 'available' && claude.readiness === 'ready' ? claude.executable : undefined
    }
    // The variables a parent Claude Code session sets for its own children: never handed on (a drive is run from one).
    const PARENT_SESSION_VARIABLES = ['CLAUDECODE', 'CLAUDE_PID', 'CLAUDE_CODE_ENTRYPOINT', 'CLAUDE_CODE_SESSION_ID', 'CLAUDE_CODE_CHILD_SESSION', 'CLAUDE_CODE_BRIDGE_SESSION_ID', 'CLAUDE_CODE_MESSAGING_SOCKET', 'CLAUDE_CODE_MESSAGING_TOKEN', 'CLAUDE_CODE_SESSION_ATTENDED', 'CLAUDE_CODE_EXECPATH']
    const claudeCommand = async (args: readonly string[], cwd: string, account?: string): Promise<{ code: number; stdout: string; stderr: string }> => {
      const executable = account === undefined ? await claudeLaunch() : (await discoverForAccount('claude', account)).find(r => r.id === 'claude' && r.readiness === 'ready')?.executable
      if (executable === undefined) return { code: 127, stdout: '', stderr: 'Claude Code is not installed or not signed in on this computer.' }
      const env = Object.fromEntries(Object.entries({ ...process.env, ...(executable.env ?? {}) }).filter(([key]) => !PARENT_SESSION_VARIABLES.includes(key)))
      return new Promise((resolve) => {
        const child = spawn(executable.executablePath, [...executable.prefixArgs, ...args], { cwd, windowsHide: true, shell: false, env, stdio: ['ignore', 'pipe', 'pipe'] })
        let stdout = ''
        let stderr = ''
        let settled = false
        // On EXIT, not close: `--bg` hands off to Claude Code's own supervisor, which need not let go of the pipes.
        const done = (code: number): void => {
          if (settled) return
          settled = true
          clearTimeout(timer)
          setTimeout(() => resolve({ code, stdout, stderr }), 50)
        }
        const timer = setTimeout(() => {
          if (process.platform === 'win32' && child.pid !== undefined) void releaseProcessTree(child.pid).catch(() => false).finally(() => child.kill())
          else child.kill()
          done(124)
        }, 60_000)
        child.stdout?.on('data', (chunk: Buffer) => { if (stdout.length < 4_000_000) stdout += chunk.toString('utf8') })
        child.stderr?.on('data', (chunk: Buffer) => { if (stderr.length < 200_000) stderr += chunk.toString('utf8') })
        child.on('error', (error) => { stderr += error.message; done(127) })
        child.on('exit', (code) => done(code ?? 1))
      })
    }
    const backgroundSessionOf = async (conversation: string, account?: string): Promise<string | undefined> => {
      const newest = await newestTurnOf(missionLedger, conversation).catch(() => conversation)
      const mission = await missionLedger.getMission(newest).catch(() => undefined)
      return mission === undefined || mission.metadata.runtime !== 'claude' || accountFromRoute(mission.metadata.resolvedRouteId) !== account ? undefined : runtimeThreadIdOf(mission)
    }
    const backgroundRuns = createBackgroundRuns({
      file: join(app.getPath('userData'), 'claude-background.json'),
      claude: claudeCommand,
      sessionOf: (conversation, account) => backgroundSessionOf(conversation, account),
      bringIn: async (run) => {
        const places = run.account === undefined ? sessionPlaces : { ...sessionPlaces, claudeHome: (await agentAccounts.state()).accounts.find(a => a.id === run.account)?.home ?? '' }
        if (!places.claudeHome) return { refused: 'The account used by this run was removed. Its record was not read from another account.' }
        const ownFacts = run.account === undefined ? catchUpFacts : { ...catchUpFacts, transcriptOf: createTranscriptReader(places) }
        if (run.conversation !== undefined) {
          const conversation = run.conversation
          /*
           * Its own session, when Claude Code forked one: MEASURED 2026-10-06, a finished background session stays
           * alive (idle), and `--resume` of a session still alive "starts a copy" under a new id -- the turn is in
           * the copy, not in the conversation's session. And a few tries: the transcript's last lines can land a
           * moment after `claude agents` says done.
           */
          const ownSession = run.sessionId
          const conversationSession = await backgroundSessionOf(conversation, run.account)
          const read = (): Promise<{ readonly imported: number; readonly latestMissionId: string }> =>
            ownSession !== undefined && ownSession !== conversationSession
              ? catchUpTerminal(conversation, { ...ownFacts, sessionOf: () => ownSession })
              : catchUpTerminal(conversation, ownFacts)
          for (let attempt = 0; attempt < 5; attempt += 1) {
            const caught = await read()
            if (caught.imported > 0) return { missionId: caught.latestMissionId }
            await new Promise((resolve) => setTimeout(resolve, 1_500))
          }
          return { refused: 'Its turn was not found in Claude Code\'s record of the conversation.' }
        }
        if (run.sessionId === undefined) return { refused: 'Claude Code never said which session it was.' }
        const imported = await importSession({ runtime: 'claude', sessionId: run.sessionId, cwd: run.folder, title: run.prompt.slice(0, 80), ...(run.account === undefined ? {} : { account: run.account }) }, {
          ledger: missionLedger,
          imports: terminalImports,
          workspaceIdFor,
          sameFolder: (cwd) => folders.sameAs(cwd),
          learnFolder: (id, path) => folders.learn([{ id, path }]),
          nameConversation: (missionId, title) => teammates.renameMission(missionId, title),
          pathOf: (runtime, sessionId) => transcriptPathFor(runtime, sessionId, places)
        })
        if (!imported.ok) return { refused: imported.message }
        if (run.teammateId !== undefined) await teammates.assignMission(run.teammateId, imported.missionId).catch(() => undefined)
        return { missionId: imported.missionId }
      },
      onChange: () => sendToWindow({ kind: 'background-changed' })
    })
    // Picked up once the window has loaded (see openMain): a run that ended while Locust was closed comes back to a
    // window that is listening, not into a message nobody hears.
    backgroundForShutdown = backgroundRuns
    const publicRun = (run: BackgroundRun): PublicBackgroundRun => {
      const { folder: _folder, sessionId: _session, ...rest } = run
      return rest
    }
    ipcMain.handle(BACKGROUND_LIST_CHANNEL, async (event) => (fromOwnWindow(event) ? (await backgroundRuns.list()).map(publicRun) : []))
    ipcMain.handle(BACKGROUND_START_CHANNEL, async (event, requested: unknown): Promise<BackgroundStartResponse> => {
      if (!fromOwnWindow(event) || typeof requested !== 'object' || requested === null) return { ok: false, message: 'That request was not understood.' }
      const asked = requested as Record<string, unknown>
      const prompt = typeof asked.prompt === 'string' ? asked.prompt : ''
      if (prompt.trim().length === 0 || prompt.length > MAX_PROMPT_LENGTH) return { ok: false, message: 'A background turn needs a message.' }
      const mode = asked.mode
      if (mode !== 'ask' && mode !== 'plan' && mode !== 'accept-edits' && mode !== 'approve-each' && mode !== 'auto') return { ok: false, message: 'That mode is not one a background turn can use.' }
      if (freeRoutesOnly(process.argv, process.env) && process.env.LOCUST_SPEND !== '1') return { ok: false, message: 'This window spends nothing, and a background turn runs Claude Code on your plan.' }
      const text = (key: string): string | undefined => (typeof asked[key] === 'string' && (asked[key] as string).length > 0 && (asked[key] as string).length <= 200 ? (asked[key] as string) : undefined)
      const conversation = text('conversation')
      const teammateId = text('teammateId')
      const model = text('model')
      const effort = text('effort')
      const account = text('account')
      const managed = await policyStartRefusal({ runtime: 'claude', mode, model, account }, teammateId)
      if (managed !== undefined) return { ok: false, message: managed }
      // Policy narrows the person's own Auto switch; it never enables it.
      if (mode === 'auto' && (await teammates.readSettings()).autoMode !== true) return { ok: false, message: 'Auto is off in Settings, so a background turn cannot use it.' }
      const started = await backgroundRuns.start({
        prompt,
        folder: workspacePath,
        sandbox: mode === 'auto' ? 'full-access' : mode === 'accept-edits' || mode === 'approve-each' ? 'workspace-write' : 'read-only',
        ...(mode === 'approve-each' ? { askEach: true } : {}),
        ...(conversation === undefined ? {} : { conversation }),
        ...(teammateId === undefined ? {} : { teammateId }),
        ...(model === undefined || model === 'account-default' ? {} : { model }),
        ...(account === undefined ? {} : { account }),
        ...(effort === undefined ? {} : { effort })
      })
      if (started.ok) note('background-started', `${started.run.id} ${mode}`)
      return started.ok ? { ok: true, run: publicRun(started.run) } : started
    })
    ipcMain.handle(BACKGROUND_STOP_CHANNEL, async (event, id: unknown) => (fromOwnWindow(event) && typeof id === 'string' ? backgroundRuns.stop(id) : false))
    ipcMain.handle(BACKGROUND_DISMISS_CHANNEL, async (event, id: unknown) => {
      if (fromOwnWindow(event) && typeof id === 'string') await backgroundRuns.dismiss(id)
    })
    const openClaudeTerminal = async (request: { readonly sessionId: string; readonly attach?: true; readonly setup?: true; readonly account?: string }) => {
      const managed = policyUnmeteredRefusal(organizationPolicy, { runtime: 'claude', mode: 'auto', account: request.account })
      if (managed !== undefined) return { ok: false, message: managed }
      const launch = request.account === undefined ? await claudeLaunch() : (await discoverForAccount('claude', request.account)).find(r => r.id === 'claude')?.executable
      if (launch === undefined) return { ok: false, message: 'Claude Code is not installed or not signed in on this computer.' }
      return openInTerminal({ runtime: 'claude', cwd: workspacePath, launch, title: 'Claude Code', ...request }, {
        ...(process.env.LOCALAPPDATA === undefined ? {} : { localAppData: process.env.LOCALAPPDATA }),
        ...(process.env.LOCUST_TERMINAL === 'console' ? { prefer: 'console' as const } : {})
      })
    }
    ipcMain.handle(BACKGROUND_OPEN_CHANNEL, async (event, id: unknown) => {
      if (!fromOwnWindow(event) || typeof id !== 'string') return { ok: false, message: 'That run cannot be opened.' }
      const run = (await backgroundRuns.list()).find((entry) => entry.id === id)
      if (run === undefined) return { ok: false, message: 'That run is not in the list any more.' }
      return openClaudeTerminal({ sessionId: id, attach: true, ...(run.account === undefined ? {} : { account: run.account }) })
    })
    ipcMain.handle(BACKGROUND_SETUP_CHANNEL, async (event) => (fromOwnWindow(event) ? openClaudeTerminal({ sessionId: 'setup', setup: true }) : { ok: false, message: 'That request was rejected.' }))
    const publicTask = <T extends { folder: string }>(task: T): Omit<T, 'folder'> => {
      const { folder: _folder, ...rest } = task
      return rest
    }
    ipcMain.handle(CLOUD_WHERE_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { repo: undefined, branch: undefined, unpushed: undefined, dirty: false, codexReady: false }
      const where = await cloudTasks.where(workspacePath)
      const codexReady = (await codexLaunch()) !== undefined
      const environment = where.repo !== undefined && codexReady ? await environmentOf(cloudAsk, where.repo, workspacePath) : undefined
      return { ...where, codexReady, folderName: basename(workspacePath), ...(environment === undefined ? {} : { environment }) }
    })
    ipcMain.handle(CLOUD_START_CHANNEL, async (event, prompt: unknown, teammateId: unknown) => {
      if (!fromOwnWindow(event) || typeof prompt !== 'string') return { ok: false, message: 'That cloud task could not be started.' }
      const managed = policyUnmeteredRefusal(organizationPolicy, { runtime: 'codex', mode: 'auto', cloud: true }) ?? await spendRefusal(typeof teammateId === 'string' ? teammateId : undefined)
      if (managed !== undefined) return { ok: false, message: managed }
      if (freeRoutesOnly(process.argv, process.env)) return { ok: false, message: FREE_ONLY_REFUSAL }
      if ((await codexLaunch()) === undefined) return { ok: false, message: 'Cloud tasks run through Codex CLI, which is not installed or not signed in here. Settings > AI agents shows how.' }
      const started = await cloudTasks.start({ folder: workspacePath, prompt: prompt.slice(0, 20_000), ...(typeof teammateId === 'string' ? { teammateId } : {}) })
      return started.ok ? { ok: true, task: publicTask(started.task), notes: started.notes } : started
    })
    /*
     * Where a cloud task CAN go, when this folder is not on GitHub (0.504).
     * Colin, 2026-09-30, at "This folder is not on GitHub": "i have no idea
     * what folder the cloud is in". The folders Locust already knows, each
     * asked for its GitHub remote -- read-only, and only those still there.
     */
    /*
     * YOUR CLOUD COMPUTER (0.761, cloud-turns.ts): a conversation's turns on the repository's GitHub Codespace, their
     * changes -- shown, brought home, put back -- and the computers themselves, for Settings.
     */
    const cloudTurns = createCloudTurns({
      file: join(app.getPath('userData'), 'cloud-turns.json'),
      computers: createCloudComputers(),
      git: gitRunner,
      onChange: (turn) => sendToWindow({ kind: 'cloud-turn', turn })
    })
    // A turn Locust closed on is read now, quietly; the window is told as each one is.
    void cloudTurns.settleLeftover().catch(() => undefined)
    const missionIdsOf = (value: unknown): readonly string[] =>
      Array.isArray(value) ? value.filter((id): id is string => typeof id === 'string' && /^mission_[A-Za-z0-9_-]{1,80}$/.test(id)).slice(0, 200) : []
    const missionIdOf = (value: unknown): string | undefined => missionIdsOf([value])[0]
    const computerName = (value: unknown): string | undefined => (typeof value === 'string' && /^[A-Za-z0-9-]{8,90}$/.test(value) ? value : undefined)
    ipcMain.handle(CLOUD_TURNS_CHANNEL, async (event, ids: unknown) => (fromOwnWindow(event) ? cloudTurns.turns(missionIdsOf(ids)) : []))
    ipcMain.handle(CLOUD_TURN_PATCH_CHANNEL, async (event, id: unknown) => {
      const missionId = missionIdOf(id)
      return fromOwnWindow(event) && missionId !== undefined ? cloudTurns.patch(missionId) : undefined
    })
    ipcMain.handle(CLOUD_TURN_APPLY_CHANNEL, async (event, id: unknown) => {
      const missionId = missionIdOf(id)
      if (!fromOwnWindow(event) || missionId === undefined) return { ok: false, message: 'That change could not be brought here.' }
      return cloudTurns.apply(missionId)
    })
    ipcMain.handle(CLOUD_TURN_DISCARD_CHANNEL, async (event, id: unknown) => {
      const missionId = missionIdOf(id)
      if (!fromOwnWindow(event) || missionId === undefined) return { ok: false, message: 'That change could not be put back.' }
      return cloudTurns.discard(missionId)
    })
    ipcMain.handle(CLOUD_TURN_PULL_REQUEST_CHANNEL, async (event, id: unknown, message: unknown) => {
      const missionId = missionIdOf(id)
      if (!fromOwnWindow(event) || missionId === undefined || typeof message !== 'string' || message.length > 20_000) {
        return { ok: false, message: 'That change could not be sent to a pull request.' }
      }
      return cloudTurns.pullRequest(missionId, message)
    })
    ipcMain.handle(CLOUD_COMPUTERS_CHANNEL, async (event) => (fromOwnWindow(event) ? cloudTurns.computers() : { ok: false, message: 'That request was rejected.' }))
    ipcMain.handle(CLOUD_COMPUTER_STOP_CHANNEL, async (event, value: unknown) => {
      const name = computerName(value)
      if (!fromOwnWindow(event) || name === undefined) return { ok: false, message: 'That cloud computer could not be stopped.' }
      return cloudTurns.stop(name)
    })
    ipcMain.handle(CLOUD_COMPUTER_REMOVE_CHANNEL, async (event, value: unknown) => {
      const name = computerName(value)
      if (!fromOwnWindow(event) || name === undefined) return { ok: false, message: 'That cloud computer could not be deleted.' }
      return cloudTurns.remove(name)
    })
    ipcMain.handle(CLOUD_COMPUTER_TERMINAL_CHANNEL, async (event, value: unknown) => {
      const name = computerName(value)
      if (!fromOwnWindow(event) || name === undefined) return { ok: false, message: 'That terminal could not be opened.' }
      const managed = policyCloudTerminalRefusal(organizationPolicy)
      if (managed !== undefined) return { ok: false, message: managed }
      return openProgramInTerminal({ file: ghCommand(), args: ['codespace', 'ssh', '-c', name] }, workspacePath, 'Your cloud computer', {
        ...(process.env.LOCALAPPDATA === undefined ? {} : { localAppData: process.env.LOCALAPPDATA })
      })
    })
    const openCodeVersionHere = async (): Promise<string | undefined> =>
      (await discoverForWork().catch(() => [])).find((entry) => entry.id === 'opencode')?.version?.version
    const agentVersionHere = async (agent: CloudAgent): Promise<string | undefined> =>
      (await discoverForWork().catch(() => [])).find((entry) => entry.id === agent)?.version?.version
    /**
     * An agent's sign-in page there, opened in the person's own browser (0.768). A drive's stand-in, labelled as such,
     * writes the page down instead: a drive is not the person, and never signs in as them. Read nowhere else.
     */
    const openSignInPage = async (url: string): Promise<void> => {
      const pages = process.env.LOCUST_SIGN_IN_PAGES
      if (pages !== undefined && pages.length > 0) {
        await appendFile(pages, `${url}
`, 'utf8')
        return
      }
      await shell.openExternal(url)
    }
    /** The one sign-in under way, which Stop ends. */
    let signingIn: AbortController | undefined
    const signInSignal = (): AbortSignal => {
      signingIn?.abort()
      signingIn = new AbortController()
      return signingIn.signal
    }
    ipcMain.handle(CLOUD_COMPUTER_FOLDER_CHANNEL, async (event) => (fromOwnWindow(event) ? cloudTurns.forFolder(workspacePath) : { ok: false, message: 'That request was rejected.' }))
    ipcMain.handle(CLOUD_COMPUTER_SETUP_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' }
      const managed = policyFeatureRefusal(organizationPolicy, 'cloud')
      if (managed !== undefined) return { ok: false, message: managed }
      const asked = typeof request === 'object' && request !== null ? (request as Record<string, unknown>).agent : undefined
      const agent = isCloudAgent(asked) ? asked : undefined
      return cloudTurns.setUp({
        folder: workspacePath,
        openCodeVersion: await openCodeVersionHere(),
        ...(agent === undefined ? {} : { agent: { runtime: agent, version: await agentVersionHere(agent) }, open: openSignInPage, signal: signInSignal() }),
        step: (step, waiting) => sendToWindow({ kind: 'cloud-setup', step, ...(waiting === true ? { waiting } : {}) })
      })
    })
    ipcMain.handle(CLOUD_COMPUTER_SIGN_IN_CHANNEL, async (event, request: unknown) => {
      const asked = typeof request === 'object' && request !== null ? (request as Record<string, unknown>) : {}
      const name = computerName(asked.name)
      const agent = isCloudAgent(asked.agent) ? asked.agent : undefined
      if (!fromOwnWindow(event) || name === undefined || agent === undefined) return { ok: false, message: 'That sign-in was not started.' }
      const managed = policyFeatureRefusal(organizationPolicy, 'cloud')
      if (managed !== undefined) return { ok: false, message: managed }
      return cloudTurns.signIn({
        name,
        agent,
        version: await agentVersionHere(agent),
        open: openSignInPage,
        step: (step) => sendToWindow({ kind: 'cloud-sign-in', name, agent, step }),
        signal: signInSignal()
      })
    })
    ipcMain.handle(CLOUD_COMPUTER_SIGN_IN_STOP_CHANNEL, async (event) => {
      if (fromOwnWindow(event)) signingIn?.abort()
    })
    ipcMain.handle(CLOUD_COMPUTER_SIGN_OUT_CHANNEL, async (event, request: unknown) => {
      const asked = typeof request === 'object' && request !== null ? (request as Record<string, unknown>) : {}
      const name = computerName(asked.name)
      const agent = isCloudAgent(asked.agent) ? asked.agent : undefined
      if (!fromOwnWindow(event) || name === undefined || agent === undefined) return { ok: false, message: 'That agent could not be signed out.' }
      return cloudTurns.signOut(name, agent)
    })
    // GitHub's codespace permission, which the person gives in the terminal and their browser: Locust never sees it.
    ipcMain.handle(CLOUD_COMPUTER_ALLOW_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' }
      const managed = policyFeatureRefusal(organizationPolicy, 'cloud')
      if (managed !== undefined) return { ok: false, message: managed }
      return openProgramInTerminal({ file: ghCommand(), args: ['auth', 'refresh', '-h', 'github.com', '-s', 'codespace'] }, workspacePath, 'Allow Locust to make codespaces', {
        ...(process.env.LOCALAPPDATA === undefined ? {} : { localAppData: process.env.LOCALAPPDATA })
      })
    })
    ipcMain.handle(CLOUD_FOLDERS_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return []
      const known = (await folders.list().catch(() => [])).filter((folder) => existsSync(folder.path) && !isInstallFolder(folder.path)).slice(0, 40)
      const found = await Promise.all(known.map(async (folder) => {
        const remote = await gitRunner(['remote', 'get-url', 'origin'], folder.path)
        const repo = remote.code === 0 ? githubRepoOf(remote.stdout) : undefined
        return repo === undefined ? undefined : { id: folder.id, name: folder.name, path: folder.path, repo }
      }))
      const onGitHub = found.filter((entry): entry is { id: string; name: string; path: string; repo: string } => entry !== undefined).slice(0, 12)
      // Each asked whether Codex Cloud has an environment for it; those that do come first (0.505).
      const withEnvironments = await Promise.all(onGitHub.map(async (entry) => ({ ...entry, environment: await environmentOf(cloudAsk, entry.repo, entry.path) })))
      const rank = { ready: 0, unknown: 1, missing: 2 } as const
      return withEnvironments.sort((left, right) => rank[left.environment] - rank[right.environment])
    })
    // Claude's cloud (0.538): Claude Code in a window of its own, with the task given.
    const claudeCloud = createClaudeCloud({ discover: discoverForWork, storePath: join(app.getPath('userData'), 'claude-cloud.json'), terminal: runInPseudoTerminal })
    const remoteControl = createRemoteControl({ discover: discoverForWork, folder: () => workspacePath })
    remoteControlForShutdown = remoteControl
    registerRemoteControlIpc(ipcMain, remoteControl, fromOwnWindow, () => policyFeatureRefusal(organizationPolicy, 'phone')
      ?? policyUnmeteredRefusal(organizationPolicy, { runtime: 'claude', mode: 'auto' })
      ?? (freeRoutesOnly(process.argv, process.env) ? FREE_ONLY_REFUSAL : undefined))
    const publicSession = <T extends { folder: string }>(session: T): Omit<T, 'folder'> => {
      const { folder: _folder, ...rest } = session
      return rest
    }
    ipcMain.handle(CLAUDE_CLOUD_START_CHANNEL, async (event, prompt: unknown, teammateId: unknown, choice: unknown) => {
      if (!fromOwnWindow(event) || typeof prompt !== 'string') return { ok: false, message: 'That could not be sent to the cloud.' }
      if (freeRoutesOnly(process.argv, process.env)) return { ok: false, message: FREE_ONLY_REFUSAL }
      // Plain strings only; claude-cloud passes a name or level and nothing else (choiceArgs).
      const picked = typeof choice === 'object' && choice !== null ? (choice as Record<string, unknown>) : {}
      const args = choiceArgs({ ...(typeof picked.model === 'string' ? { model: picked.model } : {}) })
      const managed = policyUnmeteredRefusal(organizationPolicy, { runtime: 'claude', mode: 'auto', model: args[0] === '--model' ? args[1] : undefined, cloud: true }) ?? await spendRefusal(typeof teammateId === 'string' ? teammateId : undefined)
      if (managed !== undefined) return { ok: false, message: managed }
      const started = await claudeCloud.start(workspacePath, prompt.slice(0, 20_000), typeof teammateId === 'string' ? teammateId : undefined, {
        ...(typeof picked.model === 'string' ? { model: picked.model } : {}),
        ...(typeof picked.effort === 'string' ? { effort: picked.effort } : {}),
        ...(typeof picked.environment === 'string' ? { environment: picked.environment } : {})
      })
      return started.ok ? { ok: true, session: publicSession(started.session) } : started
    })
    ipcMain.handle(CLAUDE_CLOUD_LIST_CHANNEL, async (event) => (fromOwnWindow(event) ? (await claudeCloud.list(workspacePath)).map(publicSession) : []))
    ipcMain.handle(CLAUDE_CLOUD_ENVIRONMENT_CHANNEL, async (event) => fromOwnWindow(event) ? claudeCloud.environment(workspacePath) : undefined)
    ipcMain.handle(CLAUDE_CLOUD_HOME_CHANNEL, async (event, id: unknown) => {
      if (!fromOwnWindow(event) || typeof id !== 'string') return { ok: false, message: 'That cloud session could not be brought home.' }
      const managed = policyUnmeteredRefusal(organizationPolicy, { runtime: 'claude', mode: 'auto', cloud: true })
      if (managed !== undefined) return { ok: false, message: managed }
      return claudeCloud.home(id)
    })
    ipcMain.handle(CLAUDE_CLOUD_TERMINAL_CHANNEL, async (event, id: unknown) => {
      if (!fromOwnWindow(event) || typeof id !== 'string') return { ok: false, message: 'That cloud session could not be opened. Open it on claude.ai.' }
      const managed = policyUnmeteredRefusal(organizationPolicy, { runtime: 'claude', mode: 'auto', cloud: true })
      if (managed !== undefined) return { ok: false, message: managed }
      return claudeCloud.continueInTerminal(id)
    })
    ipcMain.handle(CLAUDE_CLOUD_SEND_CHANNEL, async (event, id: unknown, message: unknown) => {
      if (!fromOwnWindow(event) || typeof id !== 'string' || typeof message !== 'string') return { ok: false, message: 'That could not be sent to the cloud session.' }
      const managed = policyUnmeteredRefusal(organizationPolicy, { runtime: 'claude', mode: 'auto', cloud: true })
      if (managed !== undefined) return { ok: false, message: managed }
      if (freeRoutesOnly(process.argv, process.env)) return { ok: false, message: FREE_ONLY_REFUSAL }
      return claudeCloud.send(id, message.slice(0, 20_000))
    })
    ipcMain.handle(CLAUDE_CLOUD_CHECK_CHANNEL, async (event, id: unknown) => {
      if (!fromOwnWindow(event) || typeof id !== 'string') return { ok: false, message: 'That cloud session could not be read.' }
      return claudeCloud.check(id)
    })
    ipcMain.handle(CLAUDE_CLOUD_APPLY_CHANNEL, async (event, id: unknown) => {
      if (!fromOwnWindow(event) || typeof id !== 'string') return { ok: false, message: 'That change could not be applied.' }
      return claudeCloud.apply(id)
    })
    ipcMain.handle(CLAUDE_CLOUD_FORGET_CHANNEL, async (event, id: unknown) => {
      if (fromOwnWindow(event) && typeof id === 'string') await claudeCloud.forget(id)
    })
    ipcMain.handle(CLOUD_LIST_CHANNEL, async (event) => (fromOwnWindow(event) ? (await cloudTasks.list(workspacePath)).map(publicTask) : []))
    ipcMain.handle(CLOUD_REFRESH_CHANNEL, async (event, taskId: unknown) => {
      if (!fromOwnWindow(event) || typeof taskId !== 'string') return undefined
      const task = await cloudTasks.refresh(taskId)
      return task === undefined ? undefined : publicTask(task)
    })
    ipcMain.handle(CLOUD_DIFF_CHANNEL, async (event, taskId: unknown) => (fromOwnWindow(event) && typeof taskId === 'string' ? cloudTasks.diff(taskId) : undefined))
    ipcMain.handle(CLOUD_APPLY_CHANNEL, async (event, taskId: unknown) => {
      if (!fromOwnWindow(event) || typeof taskId !== 'string') return { ok: false, message: 'That change could not be applied.' }
      const applied = await cloudTasks.apply(taskId)
      return applied.ok ? { ok: true, task: publicTask(applied.task) } : applied
    })

    ipcMain.handle(FOLDER_SWITCH_CHANNEL, async (event, id: unknown) => {
      if (!fromOwnWindow(event) || typeof id !== 'string') return { ok: false, message: 'That folder could not be opened.' }
      const path = await folders.pathOf(id)
      if (path === undefined) return { ok: false, message: 'Locust does not know where that folder is.' }
      if (isInstallFolder(path)) return { ok: false, message: 'That is where Locust itself is installed. Pick a project folder instead.' }
      if (!existsSync(path)) return { ok: false, message: `${path} is not there any more.` }
      const now = await switchFolder(path)
      return { ok: true, folder: now }
    })

    /*
     * Open one of the addresses this app is allowed to open, and no other.
     *
     * The policy above still holds -- `window.open` is denied and navigation
     * away from the app's URL is cancelled -- and it asked that a feature
     * needing a link name the exact URL in the HOST. The first-run panel
     * grew three links and nobody did, so every "Get it" in every shipped
     * build was dead: it looked like a link, it had a cursor, and clicking
     * it did nothing at all. Found by the first outside tester on 0.55.0,
     * who had no Node.js and whose only offered way out was one of them.
     *
     * The renderer names a URL; the host opens it only if it is on the
     * host's own list, so a renderer turned against the person can still
     * reach nowhere else.
     */
    /*
     * A NEWER LOCUST FOR THIS MAC (0.515, mac-release.ts). A Mac copy cannot
     * update itself until it is signed; it can say when one is out. Only on
     * a packaged Mac build, and nothing is downloaded here: the window offers
     * the release's own link, and the person's browser fetches it.
     */
    // What a reply on another runtime would carry (0.517): composed as the send would, nothing written.
    ipcMain.handle(HANDOFF_PREVIEW_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event) || typeof request !== 'object' || request === null) return undefined
      const { followUpOf, runtime, prompt, leaveOut } = request as Record<string, unknown>
      if (typeof followUpOf !== 'string' || !/^[A-Za-z0-9_-]{1,128}$/.test(followUpOf) || !isMissionRuntime(runtime) || typeof prompt !== 'string') return undefined
      return codexMissions.previewSwitch(followUpOf, runtime, prompt.slice(0, 20_000), chosenLeaveOut(leaveOut)).catch(() => undefined)
    })

    ipcMain.handle(MAC_RELEASE_CHANNEL, async (event) => {
      if (!fromOwnWindow(event) || process.platform !== 'darwin' || !app.isPackaged) return undefined
      // A Mac that updates itself (0.516) says so through the ordinary update banner, not this one.
      if (macSelfUpdateTarget(process.execPath) !== undefined) return undefined
      try {
        const response = await fetch(MAC_RELEASES_API, { signal: AbortSignal.timeout(10_000) })
        if (!response.ok) return undefined
        return newerMacRelease(await response.json(), app.getVersion(), process.arch)
      } catch {
        return undefined
      }
    })

    ipcMain.handle(OPEN_LINK_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      /*
       * The app's own three addresses, OR a plain web address from a reply.
       *
       * The widening is real and is named here rather than buried: before
       * this, a renderer turned against the person could reach exactly three
       * hosts, and now it can reach any http(s) one. What has not changed is
       * that `shell.openExternal` opens the PERSON's browser, visibly, and
       * only ever because they clicked something. What is refused has not
       * changed either: every non-web scheme, which is what the original
       * policy was actually defending -- a reply must not become a way to
       * reach this machine. See `isWebLink`.
       */
      if (!isOutboundLink(requested) && !isWebLink(requested)) {
        return { ok: false, message: 'Locust does not open that address.' } as const
      }
      try {
        await shell.openExternal(requested)
        return { ok: true } as const
      } catch {
        return { ok: false, message: 'Your browser could not be opened.' } as const
      }
    })

    /*
     * Reveal, never open -- the same rule `reveal-file.ts` sets out. A log is
     * a text file and opening one is harmless, but the rule is worth keeping
     * whole: the app never hands a path to the operating system and asks it
     * to decide what running it means.
     *
     * The file is guaranteed to exist by the time anyone can press this,
     * because every run writes its opening line before a window is shown.
     */
    /*
     * Send feedback, in the person's browser: the one address in
     * report-problem.ts, filled in with their words, the version, the Windows
     * build and the conversation they sent it from. The renderer hands over
     * words only; anything that is not a string is dropped.
     */
    ipcMain.handle(FEEDBACK_CHANNEL, async (event, report: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const record = (typeof report === 'object' && report !== null ? report : {}) as Record<string, unknown>
      const description = typeof record.description === 'string' ? record.description : ''
      if (description.trim().length === 0) return { ok: false, message: 'Say what happened first.' } as const
      const conversation = typeof record.conversation === 'string' ? record.conversation : undefined
      try {
        await shell.openExternal(
          feedbackUrl(
            { version: app.getVersion(), release: release(), arch: process.arch, platform: process.platform },
            { description, ...(conversation === undefined ? {} : { conversation }) }
          )
        )
        return { ok: true } as const
      } catch {
        return { ok: false, message: 'Your browser could not be opened. What you wrote is still here.' } as const
      }
    })

    /*
     * THE PRIVATE WAYS (0.593, R23). The same report as an email to the
     * support address -- the mail app opens filled in, the conversation only
     * while it fits a mailto -- or saved whole as a file the person sends
     * however they like. The host builds both, as it builds the issue.
     */
    const feedbackOf = (report: unknown): { readonly description: string; readonly conversation?: string } | undefined => {
      const record = (typeof report === 'object' && report !== null ? report : {}) as Record<string, unknown>
      const description = typeof record.description === 'string' ? record.description : ''
      if (description.trim().length === 0) return undefined
      const conversation = typeof record.conversation === 'string' ? record.conversation : undefined
      return { description, ...(conversation === undefined ? {} : { conversation }) }
    }
    const reportFacts = () => ({ version: app.getVersion(), release: release(), arch: process.arch, platform: process.platform })
    ipcMain.handle(FEEDBACK_EMAIL_CHANNEL, async (event, report: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const given = feedbackOf(report)
      if (given === undefined) return { ok: false, message: 'Say what happened first.' } as const
      try {
        await shell.openExternal(supportMailtoUrl(reportFacts(), given))
        return { ok: true } as const
      } catch {
        return { ok: false, message: 'Your mail app could not be opened. Save the report as a file instead; what you wrote is still here.' } as const
      }
    })
    ipcMain.handle(FEEDBACK_SAVE_CHANNEL, async (event, report: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const given = feedbackOf(report)
      if (given === undefined) return { ok: false, message: 'Say what happened first.' } as const
      const window = BrowserWindow.fromWebContents(event.sender)
      if (window === null) return { ok: false, message: 'That request was rejected.' } as const
      const text = reportFileText(reportFacts(), given)
      const stamp = new Date().toISOString().slice(0, 16).replace('T', ' ').replace(':', '.')
      // A drive on a development build names the file itself: a native dialog cannot be driven.
      const scripted = !app.isPackaged ? process.env.LOCUST_REPORT_PATH : undefined
      const target = scripted !== undefined && scripted.length > 0
        ? scripted
        : (await dialog.showSaveDialog(window, {
            title: 'Save the report',
            defaultPath: join(app.getPath('downloads'), `Locust report ${stamp}.txt`),
            filters: [{ name: 'Text', extensions: ['txt'] }]
          })).filePath
      if (target === undefined || target.length === 0) return { ok: true } as const
      try {
        await writeFile(target, text, 'utf8')
        return { ok: true, path: target } as const
      } catch (error) {
        return { ok: false, message: `The report could not be written there: ${error instanceof Error ? error.message : String(error)}` } as const
      }
    })
    ipcMain.handle(DIAGNOSTICS_REVEAL_CHANNEL, () => {
      shell.showItemInFolder(errorLog())
    })
    /*
     * THE FOLDER BUTTON in the conversation's header (0.797), as Claude Code's: the folder teammates work in, opened
     * in the file manager. The window names no path -- this is the host's own folder -- and a folder opens, it cannot
     * run, so `openPath` is safe here where it is not for a file a model wrote (reveal-file.ts).
     */
    ipcMain.handle(WORKSPACE_OPEN_CHANNEL, async () => {
      if (!workspaceChosen) return { ok: false, message: NO_WORKSPACE_MESSAGE } as const
      let folder = false
      try {
        folder = statSync(workspacePath).isDirectory()
      } catch {
        folder = false
      }
      if (!folder) return { ok: false, message: 'That folder is not there any more. Choose it again in the chat box.' } as const
      const failed = await shell.openPath(workspacePath)
      return failed.length === 0 ? ({ ok: true } as const) : ({ ok: false, message: failed } as const)
    })
    ipcMain.handle(DIAGNOSTICS_REPORT_CHANNEL, () => {
      const path = errorLog()
      try {
        return { path, exists: true, byteTotal: statSync(path).size }
      } catch {
        return { path, exists: false, byteTotal: 0 }
      }
    })
    ipcMain.handle(WORKSPACE_REVEAL_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      // The roots are the host's, never the renderer's. The workspace and
      // every worktree inside it, Locust's own ledger folder, and any folder
      // a teammate has been pointed at -- a file that teammate wrote is a
      // file this app showed you, and refusing to open it would be the app
      // disowning its own work. All of them are the HOST's paths; the
      // renderer still names nothing it was not already shown.
      // And a comparison column's copy (0.555): a file a column wrote there is one this app showed you.
      // And a file outside them all that a teammate's own record names (0.787, named-files.ts).
      const decision = await decideTeammateFile(requested, [
        ...(await workedInFolders()),
        ...(await teammateFolders()),
        ledgerDirectory,
        compareRoot()
      ], namedFile)
      if (!decision.ok) {
        return {
          ok: false,
          message:
            decision.reason === 'no-path'
              ? 'There is no file to show.'
              : 'That file is outside the folders your teammates work in, and no teammate wrote it or handed it over, so Locust will not open it.'
        } as const
      }
      /*
       * IS IT ACTUALLY THERE?
       *
       * `showItemInFolder` on a path that does not exist does nothing at all,
       * silently, and the card had no way to say so -- so a model that names
       * a file it never wrote produced a button that looked identical to one
       * that works. Caught by the handover drive on 2026-09-20: the reply
       * said it had written notes/summary.md, the card drew it, and the file
       * was not on disk.
       *
       * This is the same shape as the dead outbound links Grok found: a
       * refusal and a success that look the same is the defect this project
       * keeps paying for.
       */
      const there = await stat(decision.path).then(() => true).catch(() => false)
      if (!there) {
        return { ok: false, message: 'That file is not there. The teammate named it but did not write it.' } as const
      }
      // Reveals, never opens: `showItemInFolder` puts a file manager in front
      // of the person. `openPath` would run a `.bat` or a `.ps1` that a model
      // wrote, which is not a click anyone should be one step away from.
      shell.showItemInFolder(decision.path)
      return { ok: true } as const
    })

    ipcMain.handle(FILE_EDITORS_CHANNEL, (event) => {
      if (!fromOwnWindow(event)) return [] as const
      return editorsOnPath(process.platform, process.env).map((editor) => editor.id)
    })
    ipcMain.handle(CLIPBOARD_WRITE_CHANNEL, (event, text: unknown) => {
      if (!fromOwnWindow(event) || typeof text !== 'string') return false
      clipboard.writeText(text)
      return true
    })
    ipcMain.handle(FILE_OPEN_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event) || typeof requested !== 'object' || requested === null) return { ok: false, message: 'That request was rejected.' } as const
      const body = requested as { path?: unknown; how?: unknown }
      const how = body.how
      if (how !== 'vscode' && how !== 'cursor' && how !== 'default') return { ok: false, message: 'That request was rejected.' } as const
      const decision = await decideTeammateFile(body.path, [
        ...(await workedInFolders()),
        ...(await teammateFolders()),
        ledgerDirectory,
        compareRoot()
      ], namedFile)
      if (!decision.ok) {
        return { ok: false, message: 'That file is outside the folders your teammates work in, so Locust will not open it.' } as const
      }
      if (how === 'default') {
        if (!defaultAppMayOpen(decision.path)) return { ok: false, message: 'Locust opens only documents, pictures, sound and video in the default app, never a file it could run. Open this one in an editor.' } as const
        const failed = await shell.openPath(decision.path)
        return failed.length === 0 ? ({ ok: true } as const) : ({ ok: false, message: failed } as const)
      }
      const editor = editorsOnPath(process.platform, process.env).find((one) => one.id === how)
      if (editor === undefined) return { ok: false, message: 'That editor is not on PATH.' } as const
      try {
        launchEditor(editor.command, decision.path, process.platform, process.env)
      } catch (error) {
        return { ok: false, message: error instanceof Error ? error.message : String(error) } as const
      }
      return { ok: true } as const
    })

    /*
     * SAVE A COPY, which is the other half of "a teammate wrote you a file".
     *
     * Reveal answers "where is it". This answers "I want it somewhere else" --
     * Colin, 2026-09-20: the little download icon Claude Code has, for moving
     * it to another folder.
     *
     * The containment is the reveal's, exactly, and for the reason that file
     * gives: the SOURCE is a request from the renderer and is honoured only
     * inside a folder the host already knows a mission ran in, or for a file
     * a teammate's own record names (0.787, named-files.ts) -- one that
     * teammate could already read and repeat to the person itself.
     *
     * The DESTINATION is never the renderer's. It comes back from a native
     * save dialog, so the only place this can write is one the person just
     * chose by hand.
     *
     * And nothing is executed. `copyFile` moves bytes; the no-open rule in
     * `reveal-file.ts` is untouched by this.
     */
    ipcMain.handle(WORKSPACE_SAVE_COPY_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      // And a comparison column's copy (0.555): a file a column wrote there is one this app showed you.
      // And a file outside them all that a teammate's own record names (0.787, named-files.ts).
      const decision = await decideTeammateFile(requested, [
        ...(await workedInFolders()),
        ...(await teammateFolders()),
        ledgerDirectory,
        compareRoot()
      ], namedFile)
      if (!decision.ok) {
        return {
          ok: false,
          message:
            decision.reason === 'no-path'
              ? 'There is no file to save.'
              : 'That file is outside the folders your teammates work in, and no teammate wrote it or handed it over, so Locust will not copy it.'
        } as const
      }
      const window = BrowserWindow.fromWebContents(event.sender)
      if (window === null) return { ok: false, message: 'That request was rejected.' } as const
      const chosen = await dialog.showSaveDialog(window, {
        title: 'Save a copy',
        defaultPath: basename(decision.path),
        properties: ['createDirectory', 'showOverwriteConfirmation']
      })
      // Cancelled is not a failure; the card says nothing and nothing moved.
      if (chosen.canceled || chosen.filePath === undefined || chosen.filePath.length === 0) {
        return { ok: true } as const
      }
      try {
        await copyFile(decision.path, chosen.filePath)
        return { ok: true } as const
      } catch {
        return { ok: false, message: 'That file could not be copied. Nothing was changed.' } as const
      }
    })

    /*
     * An attached image, for the renderer to draw.
     *
     * The same containment as a reveal, and for the same reason: this reads
     * bytes off the disk and hands them to a web page, so the only paths it
     * will answer are ones inside the folder the teammates work in. A file
     * from outside was already copied in by the attach handler, so by the time
     * anything asks for a preview it IS inside -- there is nothing this needs
     * to reach that the picker did not already put there.
     *
     * Every refusal is quiet. The file row is drawn either way and already
     * carries the name; a preview is an extra, and an extra that fails should
     * leave no wreckage on screen.
     */
    ipcMain.handle(WORKSPACE_IMAGE_CHANNEL, async (event, requested: unknown, folder: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      return readWorkspaceImage(requested, folder ?? workspacePath, [...(await workedInFolders()), ...(await teammateFolders()), compareRoot()], namedFile)
    })

    /*
     * ONE CONNECTOR, ADDED TO EVERY AGENT CHOSEN (0.716, connector-add.ts):
     * each agent's own `mcp add`, run as discovery found that agent, after
     * asking it whether it already has one of that name. Listings can take
     * OpenCode 20 s (it connects to each server), so these have a runner of
     * their own with a longer ceiling than the health reading's.
     */
    const connectorChangeRunner = createNodeProbeRunner({ maximumTimeoutMs: 60_000 })
    const connectorLaunches = async (): Promise<(agent: ConnectorAgent) => AgentLaunch | undefined> => {
      const found = await discoverForWork()
      return (agent) => found.find((entry) => entry.id === agent)?.executable
    }
    ipcMain.handle(CONNECTOR_ADD_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const asked = typeof requested === 'object' && requested !== null ? (requested as Record<string, unknown>) : {}
      const agents = Array.isArray(asked.agents) ? asked.agents.filter(isConnectorAgent) : []
      if (typeof asked.name !== 'string' || typeof asked.value !== 'string' || (asked.kind !== 'command' && asked.kind !== 'url') || agents.length === 0) {
        return { ok: false, message: 'Name the connector, say how it runs, and choose at least one agent.' } as const
      }
      try {
        const result = await addConnector({ name: asked.name, kind: asked.kind, value: asked.value, agents }, await connectorLaunches(), connectorChangeRunner)
        // The Connectors list reads Claude Code's own listing again when it is next asked.
        if (result.ok && result.results.some((one) => one.agent === 'claude' && one.outcome === 'added')) connectorReader.forget()
        return result
      } catch {
        return { ok: false, message: 'The connector could not be added. Nothing was changed in any agent that did not say so.' } as const
      }
    })
    ipcMain.handle(CONNECTOR_REMOVE_CHANNEL, async (event, name: unknown, agents: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const chosen = Array.isArray(agents) ? agents.filter(isConnectorAgent) : []
      if (typeof name !== 'string' || name.trim().length === 0 || chosen.length === 0) return { ok: false, message: 'Nothing to take back.' } as const
      try {
        const results = await removeConnector(name.trim(), chosen, await connectorLaunches(), connectorChangeRunner)
        if (results.some((one) => one.agent === 'claude' && one.outcome === 'removed')) connectorReader.forget()
        return { ok: true, results } as const
      } catch {
        return { ok: false, message: 'It could not be taken back. Check each agent’s connectors.' } as const
      }
    })

    /*
     * YOUR GITHUB (0.720, github-account.ts): the GitHub CLI's own account.
     * Signing in runs `gh auth login --web`; its one-time code goes to the
     * window that asked, and the person enters it on GitHub's own page.
     */
    const githubAccount = createGithubAccount()
    // A sign-in still waiting when Locust quits is stopped with it, not left asking GitHub.
    app.once('will-quit', () => githubAccount.cancel())
    ipcMain.handle(GITHUB_ACCOUNT_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { kind: 'unknown', message: 'That request was rejected.' } as const
      return githubAccount.read().catch(() => ({ kind: 'unknown', message: 'The GitHub CLI could not be asked. Nothing was changed; Check again asks it once more.' }) as const)
    })
    ipcMain.handle(GITHUB_SIGN_IN_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const result = await githubAccount.signIn((code) => {
        if (!event.sender.isDestroyed()) event.sender.send(GITHUB_SIGN_IN_CODE_CHANNEL, code)
      })
      // A pull request is offered by whether gh is signed in: ask it afresh.
      folderCommits.forgetSignIn()
      return result
    })
    ipcMain.handle(GITHUB_SIGN_IN_CANCEL_CHANNEL, (event) => {
      if (fromOwnWindow(event)) githubAccount.cancel()
    })
    // Install the GitHub CLI from the card (0.724): winget, or Homebrew on a Mac that has it.
    ipcMain.handle(GITHUB_INSTALL_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.', getItYourself: false } as const
      const result = await githubAccount
        .install()
        .catch(() => ({ ok: false, message: 'The install could not be started. Nothing was installed.', getItYourself: true }) as const)
      folderCommits.forgetSignIn()
      return result
    })
    // Which gh this is and the newest released, and an update from the card (0.726).
    ipcMain.handle(GITHUB_CLI_VERSIONS_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return {}
      return githubAccount.versions().catch(() => ({}))
    })
    ipcMain.handle(GITHUB_UPDATE_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.', getItYourself: false } as const
      return githubAccount
        .update()
        .catch(() => ({ ok: false, message: 'The update could not be started. Nothing was changed.', getItYourself: true }) as const)
    })

    // An attached PDF's page pictures, for its card in the chat and the viewer (0.714, pdf-pages.ts).
    ipcMain.handle(PDF_PAGES_CHANNEL, async (event, requested: unknown, folder: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      return readPdfPages(requested, folder ?? workspacePath, [...(await workedInFolders()), ...(await teammateFolders())], pdfReadings.prepare)
    })

    /*
     * THE FILE VIEWER'S READ.
     *
     * Colin, 2026-09-20: "is there a way like what claude code has where when
     * you click a file/md it opens it over here near where our activity would
     * be if opened?" A teammate writes you a report and the most this app
     * could do was put a file manager in front of it.
     *
     * Deliberately the image handler's shape, line for line, because the
     * question it answers is the same question: may the renderer have the
     * bytes at this path. The roots are the HOST's -- the workspace, every
     * teammate folder, the ledger -- so a path is a request and never an
     * instruction, exactly as `reveal-file.ts` sets out.
     *
     * Size is checked BEFORE reading, for the reason the handler above gives:
     * reading a file to find out how big it is has already done the thing the
     * cap exists to prevent. And it REFUSES rather than truncating -- half a
     * file drawn as though it were whole is the quiet kind of lie this
     * project keeps finding in itself.
     *
     * What this does NOT do is run anything. It hands text to a renderer that
     * escapes every value, which is the same treatment a reply gets, and for
     * the same reason: a file in the workspace was written by a model.
     */
    /*
     * A web page, served to the preview frame and run there (0.425,
     * main/page-preview.ts; Colin's decision, docs/DECISION-2026-09-28-PAGE-
     * PREVIEW.md). The same folders the viewer opens files from.
     */
    const pages = createPageServer({
      // And a comparison's plain copies (0.448), whose pages each column runs.
      // And a reply's own page (0.553), each in a folder of its own.
      roots: async () => [...(await workedInFolders()), ...(await teammateFolders()), compareRoot(), REPLY_PAGE_ROOT],
      base: () => (workspaceChosen ? workspacePath : undefined)
    })
    protocol.handle(PAGE_SCHEME, (request) => pages.handle(request.url))
    // The `/` menu's runtime half (0.426).
    /*
     * OpenCode lists its commands only through its server (0.427,
     * opencode-commands.ts): asked once a session, in this folder, in the
     * background, the first time the window reads the menu's commands. A
     * list that changed is announced like Claude Code's.
     */
    /*
     * And Claude Code's the same way (0.428): 0.426 learned them only from a
     * run, so an updated app showed none until the first Claude turn (Colin,
     * 2026-09-28, looking at exactly that: "am i doing something wrong?").
     * Claude Code lists them at start and does nothing until it is sent a
     * message, which it never is (claude-commands.ts).
     */
    const commandsAsked = new Set<'claude' | 'opencode'>()
    const refreshListedCommands = (runtime: 'claude' | 'opencode'): void => {
      if (commandsAsked.has(runtime)) return
      commandsAsked.add(runtime)
      void (async () => {
        const found = (await discoverForStart(runtime)).find((entry) => entry.id === runtime)
        if (found?.availability !== 'available' || found.readiness !== 'ready' || found.executable === undefined) {
          // Not ready yet, or not here: asked again the next time.
          commandsAsked.delete(runtime)
          return
        }
        if (runtime === 'opencode' && openCodeUsesAcp(found.version)) {
          // V2 has no serve endpoint. Clear a previous V1 list and let a
          // later version change be checked again instead of caching a skip.
          commandsAsked.delete(runtime)
          if (await runtimeCommands.set(runtime, [])) sendToWindow({ kind: 'runtime-commands-changed' })
          return
        }
        const commands = runtime === 'claude'
          ? await readClaudeCommands({ spawn: spawnAppServer, command: createClaudeCommandListCommand(found.executable, { workspacePath }), onModels: adoptClaudeListed })
          : await readOpenCodeCommands({ spawn: spawnAppServer, command: createOpenCodeServeCommand(found.executable, { workspacePath }), ...(found.version === undefined ? {} : { version: found.version }) })
        if (await runtimeCommands.set(runtime, commands)) sendToWindow({ kind: 'runtime-commands-changed' })
      })().catch((error: unknown) => note('runtime-commands', `could not list ${runtime}'s commands: ${error instanceof Error ? error.message : String(error)}`))
    }
    ipcMain.handle(RUNTIME_COMMANDS_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return {}
      refreshListedCommands('claude')
      refreshListedCommands('opencode')
      return runtimeCommands.list().catch(() => ({}))
    })
    /*
     * POINT AT A PART OF A PAGE (0.484). The picker runs in the page's own
     * frame and answers with what was clicked; the part is photographed from
     * this window, never the screen, and handed back as bytes for the chat
     * box to attach the way a pasted picture is. One pick at a time.
     */
    let picking = false
    ipcMain.handle(PAGE_PICK_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const asked = typeof requested === 'object' && requested !== null ? (requested as Record<string, unknown>) : {}
      const box = typeof asked.frame === 'object' && asked.frame !== null ? (asked.frame as Record<string, unknown>) : {}
      const frameBox = { x: Number(box.x), y: Number(box.y), width: Number(box.width), height: Number(box.height) }
      if (typeof asked.pageUrl !== 'string' || !Object.values(frameBox).every(Number.isFinite)) return { ok: false, message: 'There is no page to point at.' } as const
      const frame = pageFrameOf(event.sender.mainFrame.framesInSubtree, asked.pageUrl)
      if (frame === undefined) return { ok: false, message: 'The page is not showing. Open it on the Page tab first.' } as const
      if (picking) return { ok: false, message: 'Already pointing at the page.' } as const
      picking = true
      try {
        const pick = await pickInFrame(frame)
        if (pick === undefined) return { ok: true } as const
        const where = captureRectOf(frameBox, pick.rect, event.sender.getZoomFactor())
        const image = where === undefined ? undefined : await event.sender.capturePage(where).catch(() => undefined)
        const png = image === undefined || image.isEmpty() ? undefined : new Uint8Array(image.toPNG())
        return { ok: true, picked: { selector: pick.selector, html: pick.html, htmlLength: pick.htmlLength }, ...(png === undefined ? {} : { png }) } as const
      } finally {
        picking = false
      }
    })
    ipcMain.handle(PAGE_PICK_CANCEL_CHANNEL, async (event, pageUrl: unknown) => {
      if (!fromOwnWindow(event) || typeof pageUrl !== 'string') return
      const frame = pageFrameOf(event.sender.mainFrame.framesInSubtree, pageUrl)
      await frame?.executeJavaScript(CANCEL_SCRIPT).catch(() => undefined)
    })
    ipcMain.handle(DIAGRAM_DRAW_CHANNEL, async (event, source: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'the request came from outside the window' } as const
      if (typeof source !== 'string') return { ok: false, message: 'there was no diagram to draw' } as const
      return drawDiagram(source)
    })
    ipcMain.handle(REPLY_PAGE_CHANNEL, async (event, html: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      if (typeof html !== 'string') return { ok: false, message: 'There is no page to open.' } as const
      const path = await writeReplyPage(html).catch(() => undefined)
      if (path === undefined) return { ok: false, message: 'That is not a whole web page, so it cannot run here.' } as const
      return pages.urlFor(path)
    })

    ipcMain.handle(WORKSPACE_PAGE_CHANNEL, async (event, requested: unknown, column: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      if (typeof requested !== 'string' || requested.length === 0) return { ok: false, message: 'There is no page to open.' } as const
      /*
       * A COMPARISON COLUMN'S PAGE, FROM ITS OWN COPY (0.452). Claude Code and
       * Codex name the page they wrote relative to where they ran -- the
       * column's copy -- and it was read from the folder: all three columns of
       * the 0.451 three-way design comparison said "That page is not there"
       * over pages that were there. Some tool reports name it by its absolute
       * copy path instead. Kept, the copy is gone and the page is the folder's
       * own, so either spelling
       * falls back to the folder, for the kept column alone.
       */
      const place = typeof column === 'object' && column !== null ? (column as { compareId?: unknown; slot?: unknown }) : undefined
      let pagePath = requested
      if (place !== undefined && typeof place.compareId === 'string' && (COMPARE_SLOTS as readonly unknown[]).includes(place.slot)) {
        const compare = await compares.get(place.compareId).catch(() => undefined)
        if (compare !== undefined) {
          const name = compareTreeId(compare.compareId, place.slot as CompareSlotId)
          // Both kinds of column live under ~/.locust/compare (0.493); an older git column, under the folder.
          const modernCopy = join(compareRoot(), name)
          const copy = compare.changesIn === 'copy' || await stat(modernCopy).then(() => true, () => false) ? modernCopy : join(workspacePath, COMPARE_TREES_DIRECTORY, name)
          const columnPath = comparePagePath(requested, modernCopy) ?? comparePagePath(requested, copy)
          if (columnPath === undefined) return pages.urlFor(requested)
          const inCopy = join(copy, columnPath)
          if (await stat(inCopy).then((found) => found.isFile(), () => false)) return pages.urlFor(inCopy)
          /*
           * ONLY THE KEPT COLUMN FALLS BACK TO THE FOLDER (0.571). After Keep the
           * folder's page IS the kept one's, so another column reading it showed
           * the kept game under its own name -- the blind arena run, 2026-10-03:
           * Opus's column drew Sonnet's "survive the dusk" title once Sonnet was
           * kept. A column not kept has no page left on this computer.
           */
          if (compare.kept !== undefined && compare.kept.slot !== place.slot) {
            return { ok: false, message: 'Its copy was removed when you kept another model, so this page is no longer on this computer.' } as const
          }
          pagePath = columnPath
        }
      }
      return pages.urlFor(pagePath)
    })

    ipcMain.handle(WORKSPACE_TEXT_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      if (typeof requested !== 'string' || requested.length === 0) {
        return { ok: false, message: 'There is no file to open.' } as const
      }
      const roots = [...(await workedInFolders()), ...(await teammateFolders()), ledgerDirectory, compareRoot()]
      // A link inside the folder that leads out of it is not the folder's to show (SEC-01).
      const linkedOut = 'That file is a link to somewhere outside the folder your teammates work in, so Locust will not open it.'
      /*
       * A SPREADSHEET, READ FOR ITS CELLS (0.364). Penny's budget workbook
       * was refused here with the sentence below (Research & money drive,
       * packaged 0.363); Colin: "it could also show in a viewer or as an
       * artifact like Claude code does". Same containment and the same
       * refusal-not-truncation rule as text; the grid is bounded instead.
       */
      const sheetKind = extensionOf(requested)
      /*
       * A WORD OR POWERPOINT FILE, READ FOR ITS WORDS (0.517). The same
       * containment as a spreadsheet; the host takes the words out and the
       * renderer draws them as text (shared/office-document.ts).
       */
      if (OFFICE_EXTENSIONS.has(sheetKind)) {
        const decision = await decideTeammateFile(requested, roots, namedFile)
        if (!decision.ok) {
          return { ok: false, message: 'That file is outside the folders your teammates work in, and no teammate wrote it or handed it over, so Locust will not open it.' } as const
        }
        try {
          if (!(await insideOnDisk(decision.path, readRoots(decision, roots)))) return { ok: false, message: linkedOut } as const
          const measured = await stat(decision.path)
          if (!measured.isFile()) return { ok: false, message: 'That is a folder, not a file.' } as const
          if (measured.size > MAX_OFFICE_FILE_BYTES) {
            return {
              ok: false,
              message: `That file is ${String(Math.round(measured.size / (1024 * 1024)))}MB, too big to show here. Save a copy and open it in ${sheetKind === 'pptx' ? 'PowerPoint' : 'Word'}.`
            } as const
          }
          const bytes = await readFile(decision.path)
          const document = sheetKind === 'pptx' ? readPptx(bytes) : readDocx(bytes)
          return { ok: true, path: requested, text: '', mode: 'document', document } as const
        } catch (error) {
          if (error instanceof WorkbookUnreadable) return { ok: false, message: error.message } as const
          return { ok: false, message: 'That file is not there. The teammate named it but did not write it.' } as const
        }
      }
      if (SHEET_EXTENSIONS.has(sheetKind)) {
        const decision = await decideTeammateFile(requested, roots, namedFile)
        if (!decision.ok) {
          return { ok: false, message: 'That file is outside the folders your teammates work in, and no teammate wrote it or handed it over, so Locust will not open it.' } as const
        }
        try {
          if (!(await insideOnDisk(decision.path, readRoots(decision, roots)))) return { ok: false, message: linkedOut } as const
          const measured = await stat(decision.path)
          if (!measured.isFile()) return { ok: false, message: 'That is a folder, not a file.' } as const
          if (measured.size > MAX_SHEET_FILE_BYTES) {
            return {
              ok: false,
              message: `That spreadsheet is ${String(Math.round(measured.size / 1024))}KB, too big to show here. Save a copy and open it in Excel or Sheets.`
            } as const
          }
          const bytes = await readFile(decision.path)
          const name = requested.replace(/\\/g, '/').split('/').pop() ?? requested
          const workbook = sheetKind === 'xlsx' ? readXlsx(bytes) : csvWorkbook(name, bytes.toString('utf8'), sheetKind === 'tsv' ? '\t' : ',')
          return { ok: true, path: requested, text: '', mode: 'table', workbook } as const
        } catch (error) {
          if (error instanceof WorkbookUnreadable) return { ok: false, message: error.message } as const
          return { ok: false, message: 'That file is not there. The teammate named it but did not write it.' } as const
        }
      }
      if (!isViewableText(requested)) {
        return { ok: false, message: 'Locust does not open that kind of file here.' } as const
      }
      const decision = await decideTeammateFile(requested, roots, namedFile)
      if (!decision.ok) {
        return {
          ok: false,
          message: 'That file is outside the folders your teammates work in, and no teammate wrote it or handed it over, so Locust will not open it.'
        } as const
      }
      try {
        if (!(await insideOnDisk(decision.path, readRoots(decision, roots)))) return { ok: false, message: linkedOut } as const
        const measured = await stat(decision.path)
        if (!measured.isFile()) return { ok: false, message: 'That is a folder, not a file.' } as const
        if (measured.size > MAX_TEXT_BYTES) {
          return {
            ok: false,
            message: `That file is ${String(Math.round(measured.size / 1024))}KB, too big to show here. Save a copy and open it in an editor.`
          } as const
        }
        const text = await readFile(decision.path, 'utf8')
        return { ok: true, path: requested, text, mode: viewerMode(requested) } as const
      } catch {
        return { ok: false, message: 'That file is not there. The teammate named it but did not write it.' } as const
      }
    })

    /*
     * Ctrl+V, which the picker cannot serve.
     *
     * Colin, 2026-09-10: "lets add the ability to ctrl+v a file or photo into
     * the chat." A pasted SCREENSHOT is the case that decides the shape --
     * the clipboard holds a bitmap, not a file, so there is no path anywhere
     * for the picker's route to take. The bytes come across and the host
     * writes them.
     *
     * The renderer suggests a name and the host does not trust it:
     * `attachmentDestination` already refuses anything that is not a plain
     * file name -- no separators, no colon, not `.` or `..` -- and always
     * returns a path inside the one folder Locust owns. So the worst a
     * renderer can do here is choose which name inside that folder it gets,
     * which is what a person choosing a file does anyway.
     */
    ipcMain.handle(WORKSPACE_PASTE_CHANNEL, async (event, requested: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      if (!workspaceChosen) {
        return { ok: false, message: 'Choose the folder your teammates work in first.' } as const
      }
      const asked = typeof requested === 'object' && requested !== null ? (requested as Record<string, unknown>) : {}
      const bytes = asked.bytes
      const name = typeof asked.name === 'string' ? asked.name : ''
      if (!(bytes instanceof Uint8Array) || bytes.byteLength === 0) {
        return { ok: false, message: 'There was nothing on the clipboard to attach.' } as const
      }
      // A bound, because this crosses IPC in one message and a clipboard can
      // hold something very large. Generous for a screenshot or a document,
      // and far short of anything that would stall the renderer.
      if (bytes.byteLength > MAX_PASTED_BYTES) {
        return {
          ok: false,
          message: 'That is too large to paste. Attach it with the + button instead.'
        } as const
      }
      try {
        const taken = new Set(await readAttachmentNames(workspacePath))
        const destination = attachmentDestination(name, taken)
        await mkdir(join(workspacePath, ATTACHMENT_DIR), { recursive: true })
        await writeFile(join(workspacePath, destination), bytes)
        await keepAttachmentsOutOfGit(workspacePath)
        // Copied in, like any file from outside: it never was in the folder,
        // and the tile says so for the same reason.
        readAttachedPdfs(workspacePath, [destination])
        return { ok: true, paths: [destination], copied: [destination] } as const
      } catch (error) {
        return {
          ok: false,
          message: error instanceof Error ? error.message : 'That could not be attached.'
        } as const
      }
    })

    // The project's files for `@` in the composer (0.436): only this window's
    // folder, listed by the host; the renderer names no folder.
    ipcMain.handle(WORKSPACE_FILES_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      if (!workspaceChosen) return { ok: false, message: 'Choose the folder your teammates work in first.' } as const
      try {
        const listed = await listWorkspaceFiles(workspacePath)
        return { ok: true, paths: listed.paths, truncated: listed.truncated } as const
      } catch {
        return { ok: false, message: "The folder's files could not be listed." } as const
      }
    })
    ipcMain.handle(WORKSPACE_ATTACH_CHANNEL, async (event) => {
      const owner = BrowserWindow.fromWebContents(event.sender)
      if (owner === null || !fromOwnWindow(event)) {
        return { ok: false, message: 'That request was rejected.' } as const
      }
      if (!workspaceChosen) {
        return { ok: false, message: 'Choose the folder your teammates work in first.' } as const
      }
      /*
       * A drive cannot put a hand in a native file dialog, and `attachFiles`
       * cannot be stubbed from the page either -- contextBridge exposes the
       * bridge non-configurable, so a drive that tried got "Cannot redefine
       * property: desktop". So a drive says which paths the dialog would have
       * returned, the same way `LOCUST_HIDE_RUNTIMES` stands in for a machine
       * with nothing installed.
       *
       * It is not a way past anything: the containment below runs on these
       * paths exactly as it runs on a person's, so this can still only ever
       * attach a file inside the workspace.
       */
      const scripted = process.env.LOCUST_ATTACH_PATHS
      const picked = scripted !== undefined && scripted.length > 0
        ? { canceled: false, filePaths: scripted.split(';').filter((entry) => entry.length > 0) }
        : await dialog.showOpenDialog(owner, {
            title: 'Attach files',
            buttonLabel: 'Attach',
            // Opens in the workspace because that is where most attachments
            // are. Anywhere else is fine now -- a file from outside is copied
            // in rather than refused, which is what the handler below does.
            defaultPath: workspacePath,
            properties: ['openFile', 'multiSelections']
          })
      if (picked.canceled || picked.filePaths.length === 0) {
        return { ok: false, message: '' } as const
      }
      /*
       * A file from outside the folder is COPIED IN rather than refused.
       *
       * It used to be refused flat, and Colin sent a screenshot of that
       * sentence: "we should definitely be able to share photos or attach
       * stuff outside of the folder much like claude." The refusal was not
       * wrong about the runtimes -- measured, only Cursor will read an
       * absolute path outside its folder; Claude, Copilot and OpenCode all
       * decline it -- so the fix is to bring the file to where every one of
       * them can already read it, not to point them somewhere they cannot go.
       *
       * See `attach-outside.ts` for why it lands in one named folder and why
       * that folder is kept out of git.
       */
      const chosenPaths = picked.filePaths.slice(0, MAX_ATTACHMENTS)
      const inside: string[] = []
      const copiedIn: string[] = []
      const taken = new Set(await readAttachmentNames(workspacePath))
      let failed = 0
      for (const chosen of chosenPaths) {
        const decision = decideReveal(chosen, [workspacePath])
        if (decision.ok) {
          inside.push(relative(workspacePath, decision.path).split('\\').join('/'))
          continue
        }
        try {
          const destination = attachmentDestination(chosen, taken)
          taken.add(basename(destination))
          await mkdir(join(workspacePath, ATTACHMENT_DIR), { recursive: true })
          // `copyFile` rather than a read-then-write: it is one syscall, it
          // keeps the bytes exactly (these are images as often as text), and
          // the destination is already known not to exist.
          await copyFile(chosen, join(workspacePath, destination))
          await keepAttachmentsOutOfGit(workspacePath)
          inside.push(destination)
          copiedIn.push(destination)
        } catch {
          failed += 1
        }
      }
      if (inside.length === 0) {
        return {
          ok: false,
          message:
            failed === 0
              ? 'Nothing was attached.'
              : 'Those files could not be copied into the folder your teammates work in.'
        } as const
      }
      // A PDF starts being opened for the agent now, so Send rarely waits (0.713).
      readAttachedPdfs(workspacePath, inside)
      return {
        ok: true,
        paths: inside,
        // Which ones came from outside, so the composer can mark those tiles.
        // Not a sentence any more: the fact is durable and belongs on the
        // durable object. See AttachFilesResponse.
        ...(copiedIn.length === 0 ? {} : { copied: copiedIn })
      } as const
    })

    ipcMain.handle(WORKSPACE_SETTINGS_WRITE_CHANNEL, async (event, settings: unknown) => {
      if (!fromOwnWindow(event)) return { swarm: false, relay: true, relayHopCap: DEFAULT_RELAY_HOP_CAP, memoryMode: DEFAULT_MEMORY_MODE, autoMode: false, askConnectors: false, keepATodoList: false, layout: 'auto', tube: 'full', replySize: 'standard' } as const
      try {
        return forThisFolder(await teammates.writeSettings(await withThisFolder(settings)))
      } catch {
        /*
         * The write failed, so answer with what is ACTUALLY STORED.
         *
         * This used to return hardcoded defaults, which is worse than
         * unhelpful: the renderer sets its switches from whatever comes back,
         * so a failed write told it relay was ON and Auto was OFF regardless
         * of the file on disk. Someone running with relay off would watch it
         * appear to switch itself on, and then switch back at the next launch
         * when the real file was read again.
         *
         * The settings on disk are the truth whether or not this write landed.
         * If even reading them fails the defaults are all that is left, and
         * that is the one case where inventing them is the honest answer --
         * there is nothing else to say.
         */
        return await teammates.readSettings().catch(() => ({
          swarm: false,
          relay: true,
          relayHopCap: DEFAULT_RELAY_HOP_CAP,
          memoryMode: DEFAULT_MEMORY_MODE,
          autoMode: false,
          askConnectors: false,
          keepATodoList: false,
          layout: 'auto'
        } as const))
      }
    })

    const listTeammates = async () => {
      try {
        const [list, missionOwners, missionTitles, missionPins, missionSettled] = await Promise.all([
          teammates.list(),
          teammates.missionOwners(),
          teammates.missionTitles(),
          teammates.missionPins(),
          teammates.missionSettled()
        ])
        return { ok: true, data: { teammates: list, missionOwners, missionTitles, missionPins, missionSettled } } as const
      } catch {
        return teammatesUnavailable
      }
    }
    ipcMain.handle(TEAMMATE_LIST_CHANNEL, (event) => fromOwnWindow(event) ? listTeammates() : teammatesUnavailable)

    ipcMain.handle(TEAMMATE_CREATE_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return teammateRejected('The teammate could not be created.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      const managed = isTeammateRoute(input.route) ? policyTurnRefusal(organizationPolicy, input.route) : undefined
      if (managed !== undefined) return teammateRejected(managed)
      try {
        // roleTitle was dropped here since Custom teammates got titles: every
        // one read "Custom" on the sidebar and in the brief (found 2026-09-05).
        const teammate = await teammates.create({ name: input.name, hue: input.hue, role: input.role, roleTitle: input.roleTitle, worktree: input.worktree, avatar: input.avatar, monthlyLimitUsd: input.monthlyLimitUsd, starters: input.starters, instructions: input.instructions, voice: input.voice })
        // The model picked on the dialog's Model row, kept exactly as a
        // started mission keeps one (rememberRoute validates it).
        if (!isTeammateRoute(input.route)) return { ok: true, data: { teammate } } as const
        await teammates.rememberRoute(teammate.teammateId, input.route)
        return { ok: true, data: { teammate: { ...teammate, route: input.route } } } as const
      } catch (error) {
        // The store's own validation is the authority; the renderer is told
        // that it was refused, never why in terms it could probe -- except a
        // name another teammate has (A2.18), which the window already knows
        // and which is the one refusal a person can fix from the sentence.
        if (error instanceof TeammateNameTakenError) return teammateRejected(error.message)
        if (error instanceof OrganizationPolicyError) return teammateRejected(error.message)
        // The real cause, when it is the file (R26): "Check the name, hue and
        // role" was said for a roster that would not read at all.
        const rosterReads = await teammates.list().then(() => true, () => false)
        if (!rosterReads) return teammateRejected('Your teammates file could not be read, so no teammate can be added until it reads again. Nothing was changed.')
        return teammateRejected('That teammate could not be created. Check the name, hue and role.')
      }
    })

    ipcMain.handle(TEAMMATE_UPDATE_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return teammateRejected('The teammate could not be updated.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      const managed = isTeammateRoute(input.route) ? policyTurnRefusal(organizationPolicy, input.route) : undefined
      if (managed !== undefined) return teammateRejected(managed)
      try {
        const teammate = await teammates.update({
          teammateId: input.teammateId,
          name: input.name,
          hue: input.hue,
          role: input.role,
          roleTitle: input.roleTitle,
          worktree: input.worktree,
          avatar: input.avatar,
          monthlyLimitUsd: input.monthlyLimitUsd,
          instructions: input.instructions,
          voice: input.voice
        })
        // A model picked on the Model row (0.311): until then a teammate's
        // model changed only when a message was sent to them on another.
        if (!isTeammateRoute(input.route)) return { ok: true, data: { teammate } } as const
        await teammates.rememberRoute(teammate.teammateId, input.route)
        return { ok: true, data: { teammate: { ...teammate, route: input.route } } } as const
      } catch (error) {
        if (error instanceof TeammateNameTakenError) return teammateRejected(error.message)
        if (error instanceof OrganizationPolicyError) return teammateRejected(error.message)
        return teammateRejected('That teammate could not be updated. Check the name, hue and role.')
      }
    })

    ipcMain.handle(TEAMMATE_SPEND_CHANNEL, async (event) => {
      const unavailable = { ok: false, error: { code: 'TEAMMATES_UNAVAILABLE', message: 'What your teammates spent could not be read.' } } as const
      if (!fromOwnWindow(event)) return unavailable
      try {
        const month = monthOf(new Date())
        const totals = month === undefined ? undefined : await spendByTeammate(missionLedger, await teammates.missionOwners(), month)
        return { ok: true, data: { byTeammate: Object.fromEntries(totals ?? []) } } as const
      } catch {
        return unavailable
      }
    })

    // Usage across every agent and model (0.714, shared/usage.ts): every turn
    // the ledger holds, added up for the range asked, on this machine's clock.
    ipcMain.handle(USAGE_READ_CHANNEL, async (event, range: unknown) => {
      const unavailable = { ok: false, message: 'Usage could not be read from the record on this machine.' } as const
      if (!fromOwnWindow(event) || !isUsageRange(range)) return unavailable
      try {
        const turns = await usageTurns(missionLedger)
        if (turns === undefined) return unavailable
        return { ok: true, summary: usageSummary(turns, range, new Date()) } as const
      } catch {
        return unavailable
      }
    })

    // A teammate's keys (0.833): their names in the window, a value going in once, from this computer's own window only.
    const NO_KEYS = { ok: false, message: 'Keys are changed only in Locust on this computer.', names: [] } as const
    ipcMain.handle(TEAMMATE_KEYS_LIST_CHANNEL, async (event, teammateId: unknown) => fromOwnWindow(event) && typeof teammateId === 'string' ? { ok: true, names: await teammateKeys.names(teammateId) } : NO_KEYS)
    ipcMain.handle(TEAMMATE_KEYS_SET_CHANNEL, (event, teammateId: unknown, name: unknown, value: unknown) => fromOwnWindow(event) ? teammateKeys.set(teammateId, name, value) : NO_KEYS)
    ipcMain.handle(TEAMMATE_KEYS_REMOVE_CHANNEL, (event, teammateId: unknown, name: unknown) => fromOwnWindow(event) ? teammateKeys.remove(teammateId, name) : NO_KEYS)
    ipcMain.handle(TEAMMATE_REMOVE_CHANNEL, async (event, teammateId: unknown) => {
      if (!fromOwnWindow(event)) return teammateRejected('The teammate could not be removed.')
      try {
        await teammates.remove(teammateId)
        // Its own browser sign-ins go with it (0.839).
        if (typeof teammateId === 'string') await browserHost.forgetProfile(teammateId).catch(() => undefined)
        // Their routines had nobody left to run them.
        await routines.removeForTeammate(teammateId).catch(() => undefined)
        // And they leave every room; a room left empty goes with them.
        await rooms.removeTeammate(teammateId).catch(() => undefined)
        await tuneLessons.removeForTeammate(teammateId).catch(() => undefined)
        await teammateSkills.removeForTeammate(teammateId).catch(() => undefined)
        // And its keys go with it (0.833).
        if (typeof teammateId === 'string') await teammateKeys.forget(teammateId).catch(() => undefined)
        return { ok: true, data: {} } as const
      } catch {
        return teammateRejected('That teammate could not be removed.')
      }
    })

    /*
     * PETS, TEAMMATES' OTHER FACES (0.563, pet-library.ts). Locust's own pet
     * beside the app (resources/pets), the ones the person adds in their
     * profile, and Codex's where Codex keeps them, read only. The gallery is
     * reached only through these handlers, and a pet is downloaded only by
     * PET_ADD_CHANNEL -- the person's click on Add.
     */
    const pets = createPetLibrary({
      installedRoot: join(app.getPath('userData'), 'pets'),
      cacheRoot: join(app.getPath('userData'), 'pets-cache'),
      bundledRoot: app.isPackaged ? join(process.resourcesPath, 'pets') : join(__dirname, '../../resources/pets'),
      codexRoot: join(process.env.CODEX_HOME ?? join(homedir(), '.codex'), 'pets'),
      /*
       * Node's own fetch, not the window session's (net.fetch): a packaged
       * build cancels every request through that session but the page
       * preview's (above), which is the renderer's egress rule -- and the
       * gallery could not be reached at all in 0.563's packaged drive. The
       * host's own reads go out as the update check's do, held instead to
       * pet-library.ts's rules: openpets.dev under /pets/, no redirect.
       */
      fetch: (url, init) => fetch(url, init),
      log: (message) => note('pets', message)
    })
    const petFailure = (error: unknown, fallback: string) =>
      ({ ok: false, error: { code: 'PETS_UNAVAILABLE', message: error instanceof PetError ? error.message : fallback } }) as const
    const petRefused = { ok: false, error: { code: 'REFUSED', message: 'That request was rejected.' } } as const

    ipcMain.handle(PET_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return petRefused
      try {
        const [list, removed] = await Promise.all([pets.list(), pets.removedPicks()])
        return { ok: true, data: { pets: list, removed } } as const
      } catch (error) {
        return petFailure(error, 'The pets on this computer could not be read.')
      }
    })

    ipcMain.handle(PET_SHEET_CHANNEL, async (event, source: unknown, id: unknown) => {
      if (!fromOwnWindow(event)) return petRefused
      const ref = { source, id }
      if (!isPetRef(ref)) return petRefused
      try {
        return { ok: true, data: await pets.sheet(ref) } as const
      } catch (error) {
        return petFailure(error, 'That pet could not be read.')
      }
    })

    ipcMain.handle(PET_THUMBNAIL_CHANNEL, async (event, id: unknown) => {
      if (!fromOwnWindow(event) || typeof id !== 'string') return petRefused
      try {
        return { ok: true, data: { dataUrl: await pets.thumbnail(id) } } as const
      } catch (error) {
        return petFailure(error, 'That pet’s picture could not be read.')
      }
    })

    ipcMain.handle(PET_ADD_CHANNEL, async (event, id: unknown) => {
      if (!fromOwnWindow(event) || typeof id !== 'string') return petRefused
      try {
        return { ok: true, data: { pet: await pets.add(id) } } as const
      } catch (error) {
        return petFailure(error, 'That pet could not be added.')
      }
    })

    ipcMain.handle(PET_REMOVE_CHANNEL, async (event, id: unknown) => {
      if (!fromOwnWindow(event) || typeof id !== 'string') return petRefused
      try {
        /*
         * Out of the picker either way (0.569, Colin: "give me an option to
         * delete some of them"). A pet a teammate wears keeps its files, so
         * that teammate's face stays until they get another look.
         */
        const wearers = (await teammates.list()).filter((teammate) => teammate.avatar.pet?.source === 'gallery' && teammate.avatar.pet.id === id)
        await pets.remove(id, wearers.length > 0)
        return { ok: true, data: { keptFor: wearers.map((teammate) => teammate.name) } } as const
      } catch (error) {
        return petFailure(error, 'That pet could not be removed.')
      }
    })

    ipcMain.handle(TEAMMATE_ASSIGN_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return teammateRejected('The mission could not be assigned.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        await teammates.assignMission(input.teammateId, input.missionId)
        return { ok: true, data: {} } as const
      } catch {
        return teammateRejected('That mission could not be assigned.')
      }
    })

    ipcMain.handle(TEAMMATE_RENAME_MISSION_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return teammateRejected('The conversation could not be renamed.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        await teammates.renameMission(String(input.missionId ?? ''), String(input.title ?? ''))
        return { ok: true, data: {} } as const
      } catch {
        return teammateRejected('That conversation could not be renamed.')
      }
    })
    // Settled out of the sidebar, snoozed until a time, or brought back (0.730).
    ipcMain.handle(TEAMMATE_SETTLE_MISSION_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return teammateRejected('The conversation could not be put away.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        await teammates.settleMission(
          String(input.missionId ?? ''),
          input.back === true ? undefined : typeof input.until === 'string' ? { until: input.until } : {}
        )
        return { ok: true, data: {} } as const
      } catch {
        return teammateRejected('That conversation could not be put away.')
      }
    })
    // Pinned to the top of the sidebar, or not (0.729).
    ipcMain.handle(TEAMMATE_PIN_MISSION_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return teammateRejected('The conversation could not be pinned.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        await teammates.pinMission(String(input.missionId ?? ''), input.pinned === true)
        return { ok: true, data: {} } as const
      } catch (error) {
        return teammateRejected(error instanceof Error && /^At most/.test(error.message) ? `${error.message}.` : 'That conversation could not be pinned.')
      }
    })

    /*
     * Groups. The store answers; this layer only decides who may ask.
     *
     * Each mutation refuses with a sentence rather than throwing, and a read
     * that cannot be done says the file could not be READ rather than
     * returning an empty list -- an empty store and an unreadable one are
     * different facts, and conflating them is how a rename once told
     * somebody a room did not exist while the room was on their screen.
     */
    const groupRejected = (message: string) => ({ ok: false, error: { code: 'GROUP_REJECTED', message } }) as const

    ipcMain.handle(GROUP_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return groupRejected('Groups are unavailable.')
      try {
        return { ok: true, data: await groups.list() } as const
      } catch {
        return { ok: false, error: { code: 'GROUPS_UNAVAILABLE', message: 'Groups could not be read.' } } as const
      }
    })

    ipcMain.handle(QUEUED_MESSAGES_READ_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'Saved messages are unavailable.' }
      try { return { ok: true, rows: await queuedMessages.list() } }
      catch { return { ok: false, message: 'Saved messages could not be read. Reopen Locust to try again; the saved file has been kept.' } }
    })
    ipcMain.handle(QUEUED_MESSAGES_WRITE_CHANNEL, async (event, rows: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'Saved messages are unavailable.' }
      try { return { ok: true, rows: await queuedMessages.replace(rows) } }
      catch { return { ok: false, message: 'This queue change was not saved. Your queued messages are still here; try again.' } }
    })

    ipcMain.handle(GROUP_CREATE_CHANNEL, async (event, name: unknown) => {
      if (!fromOwnWindow(event)) return groupRejected('The group could not be created.')
      try {
        await groups.create(name)
        return { ok: true, data: {} } as const
      } catch (error) {
        return groupRejected(error instanceof Error ? error.message : 'That group could not be created.')
      }
    })

    ipcMain.handle(GROUP_RENAME_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return groupRejected('The group could not be renamed.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        await groups.rename(input.groupId, input.name)
        return { ok: true, data: {} } as const
      } catch (error) {
        return groupRejected(error instanceof Error ? error.message : 'That group could not be renamed.')
      }
    })

    ipcMain.handle(GROUP_REMOVE_CHANNEL, async (event, groupId: unknown) => {
      if (!fromOwnWindow(event)) return groupRejected('The group could not be removed.')
      try {
        await groups.remove(groupId)
        return { ok: true, data: {} } as const
      } catch {
        return groupRejected('That group could not be removed.')
      }
    })

    ipcMain.handle(GROUP_ASSIGN_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return groupRejected('The conversation could not be moved.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        await groups.assign(input.missionId, input.groupId)
        return { ok: true, data: {} } as const
      } catch (error) {
        return groupRejected(error instanceof Error ? error.message : 'That conversation could not be moved.')
      }
    })

    ipcMain.handle(GROUP_INSTRUCTIONS_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return groupRejected('The instructions could not be saved.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        await groups.setInstructions(input.groupId, input.instructions)
        return { ok: true, data: {} } as const
      } catch (error) {
        return groupRejected(error instanceof Error ? error.message : 'Those instructions could not be saved.')
      }
    })

    ipcMain.handle(GROUP_ROUTE_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return groupRejected('The route could not be saved.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        await groups.setRoute(input.groupId, input.route)
        return { ok: true, data: {} } as const
      } catch (error) {
        return groupRejected(error instanceof Error ? error.message : 'That route could not be saved.')
      }
    })

    const routineRejected = (message: string) => ({ ok: false, error: { code: 'ROUTINE_REJECTED', message } }) as const

    // Rooms: a named set of teammates a person writes to at once. A post
    // starts one ordinary mission per teammate on that teammate's own
    // route, owned by them; the room remembers which. Nothing new reaches
    // the ledger (docs/FEATURES-FROM-VISION-2026-09-05.md, #2: "a surface,
    // not a new engine").
    const roomRejected = (message: string) => ({ ok: false, error: { code: 'ROOM_REJECTED', message } }) as const

    ipcMain.handle(ROOM_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, error: { code: 'ROOMS_UNAVAILABLE', message: 'Rooms are unavailable.' } } as const
      try {
        return { ok: true, data: { rooms: await rooms.list() } } as const
      } catch {
        return { ok: false, error: { code: 'ROOMS_UNAVAILABLE', message: 'Rooms could not be read.' } } as const
      }
    })

    ipcMain.handle(ROOM_CREATE_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return roomRejected('The room could not be created.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        // Only teammates on the roster can be in a room.
        const roster = await teammates.list()
        const wanted = Array.isArray(input.teammateIds) ? input.teammateIds : []
        const known = wanted.filter((id) => roster.some((entry) => entry.teammateId === id))
        if (known.length !== wanted.length) return roomRejected('Every teammate in a room has to be on the roster.')
        const room = await rooms.create({ name: input.name, teammateIds: known })
        return { ok: true, data: { room } } as const
      } catch (error) {
        return roomRejected(error instanceof Error ? error.message : 'That room could not be created.')
      }
    })

    ipcMain.handle(ROOM_REMOVE_CHANNEL, async (event, roomId: unknown) => {
      if (!fromOwnWindow(event)) return roomRejected('The room could not be removed.')
      if (typeof roomId !== 'string') return roomRejected('That room could not be removed.')
      try {
        await rooms.remove(roomId)
        return { ok: true, data: {} } as const
      } catch {
        return roomRejected('That room could not be removed.')
      }
    })

    ipcMain.handle(ROOM_RENAME_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return roomRejected('The room could not be renamed.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      if (typeof input.roomId !== 'string') return roomRejected('That room could not be renamed.')
      try {
        // The store validates the name and says what is wrong with it, so its
        // words reach the person rather than a sentence made up here.
        return { ok: true, data: { room: await rooms.rename(input.roomId, input.name) } } as const
      } catch (error) {
        return roomRejected(error instanceof Error ? error.message : 'That room could not be renamed.')
      }
    })

    ipcMain.handle(ROOM_TASK_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return roomRejected('The board could not be changed.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      const op = input.op
      if (op !== 'add' && op !== 'assign' && op !== 'done' && op !== 'reopen' && op !== 'remove') {
        return roomRejected('That is not a way to move a task.')
      }
      if (typeof input.roomId !== 'string') return roomRejected('That room could not be found.')
      try {
        const room = await rooms.updateTask({
          roomId: input.roomId,
          op,
          ...(typeof input.taskId === 'string' ? { taskId: input.taskId } : {}),
          ...(typeof input.text === 'string' ? { text: input.text } : {}),
          ...(typeof input.ownerId === 'string' ? { ownerId: input.ownerId } : {})
        })
        return { ok: true, data: { room } } as const
      } catch (error) {
        return roomRejected(error instanceof Error ? error.message : 'The board could not be changed.')
      }
    })

    /*
     * A FINISHED ANSWER, READ ONCE (0.370).
     *
     * Every member of a post is told the room's earlier answers, and a post
     * asks up to eight members: read afresh each time, one post would recover
     * the same thirty-odd missions from disk eight times over. A finished
     * answer never changes, so it is kept; one still running is read again
     * next time, and is not told until it finishes.
     */
    const finishedAnswers = new Map<string, string>()
    const finishedAnswerOf = async (missionId: string): Promise<string | undefined> => {
      const known = finishedAnswers.get(missionId)
      if (known !== undefined) return known
      const recovered = await missionLedger.getMission(missionId).catch(() => undefined)
      if (recovered === undefined) return undefined
      const tracker = createTranscriptTracker()
      tracker.track(recovered.events)
      const text = tracker.latestFinal
      if (!tracker.completed || text === undefined) return undefined
      if (finishedAnswers.size >= 256) {
        const oldest = finishedAnswers.keys().next()
        if (oldest.done !== true) finishedAnswers.delete(oldest.value)
      }
      finishedAnswers.set(missionId, text)
      return text
    }

    /**
     * The room before `postId`: its last few posts and the answers to them
     * that finished (shared/room-history.ts). A post that is not in `room`
     * yet -- the post handler read the room before adding it -- comes after
     * every post that is.
     */
    const roomHistoryOf = async (room: PublicRoom, postId: string, roster: readonly PublicTeammate[]): Promise<RoomHistory> => {
      const at = room.posts.findIndex((entry) => entry.postId === postId)
      const earlier = (at === -1 ? room.posts : room.posts.slice(0, at)).slice(-ROOM_HISTORY_POSTS)
      const posts = await Promise.all(
        earlier.map(async (entry) => {
          const answers = await Promise.all(
            Object.entries(entry.missions).map(async ([teammateId, missionId]): Promise<RoomHistoryAnswer | undefined> => {
              const text = await finishedAnswerOf(missionId)
              if (text === undefined) return undefined
              // Said by someone who has since left: still said, and not by nobody.
              const name = roster.find((mate) => mate.teammateId === teammateId)?.name ?? 'A teammate no longer on the team'
              return { teammateId, name, text }
            })
          )
          // Who it was put to, when the person named someone: the others were not asked.
          const to = entry.to?.map((teammateId) => ({
            teammateId,
            name: roster.find((mate) => mate.teammateId === teammateId)?.name ?? 'a teammate no longer on the team'
          }))
          return {
            text: entry.text,
            ...(to === undefined || to.length === 0 ? {} : { to }),
            answers: answers.filter((answer): answer is RoomHistoryAnswer => answer !== undefined)
          }
        })
      )
      return { roomName: room.name, posts }
    }

    /**
     * Start one room member's mission, or say why not.
     *
     * Pulled out of the post handler so the QUEUE can use it too: a member
     * who had no slot when the post went out is started by exactly this
     * path when one frees, on the same route, with the same briefing.
     *
     * `retryable` is the whole point of the split. RUN_ALREADY_ACTIVE means
     * "not now" -- the live cap is full, or that teammate is already working
     * -- and waiting fixes both. Everything else (gone from the roster,
     * Antigravity, approve-each) is a refusal that waiting will not fix, and
     * only those are recorded against the post as refused.
     */
    const startRoomMember = async (
      room: PublicRoom,
      teammateId: string,
      post: { readonly postId: string; readonly text: string },
      roster: readonly PublicTeammate[]
    ): Promise<
      | { readonly ok: true; readonly data: CodexMissionStartData }
      | { readonly ok: false; readonly name: string; readonly message: string; readonly retryable: boolean; readonly pool?: boolean }
    > => {
      const text = post.text
      const teammate = roster.find((entry) => entry.teammateId === teammateId)
      const briefedAs = await peerContextFor(teammateId)
      if (teammate === undefined || briefedAs === undefined) {
        return { ok: false, name: teammate?.name ?? teammateId, message: 'No longer on the roster.', retryable: false }
      }
      // A room is a conversation (0.370): the member is told what was said
      // before this post. A room whose record cannot be read is told nothing
      // more than the post, as before -- never a refusal to answer it.
      const history = await roomHistoryOf(room, post.postId, roster).catch(() => undefined)
      const peer = history === undefined || history.posts.length === 0 ? briefedAs : { ...briefedAs, roomHistory: history }
      const route = teammate.route ?? { runtime: 'codex' as const, model: 'account-default', mode: 'ask' as const }
      // The person's words, then the board: which room this is, who else
      // is in it, every task as it stands, and how to move one. The room
      // keeps only the person's words as the post; this trailer is what
      // the mission is briefed with. Read fresh, so a member started from
      // the queue is briefed with the board as it stands NOW.
      const memberNames = room.teammateIds.map((id: string) => roster.find((entry) => entry.teammateId === id)?.name ?? id)
      // Fitted beside the post: the post goes whole and the board gives way,
      // or a long post was refused for every member (room-task.ts).
      const board = fittedTaskSection({
        roomName: room.name,
        selfName: teammate.name,
        memberNames,
        tasks: boardLines(room.tasks, roster),
        budget: MAX_PROMPT_LENGTH - text.length - 2
      })
      const briefed = board.length === 0 ? text : `${text}\n\n${board}`
      if (route.runtime === 'antigravity' && antigravityProbe.cliPath() === undefined) {
        return { ok: false, name: teammate.name, message: 'Antigravity can be posted to from a room once Antigravity CLI is installed.', retryable: false }
      }
      const response = await codexMissions.start(
        briefed,
        route.runtime,
        route.mode,
        // M10: the teammate's saved effort too, as a direct message runs them.
        { ...(route.model === 'account-default' ? {} : { model: route.model }), ...(route.account === undefined ? {} : { account: route.account }), ...(route.effort === undefined ? {} : { effort: route.effort }) },
        sendToWindow,
        undefined,
        peer,
        undefined,
        undefined,
        undefined
      )
      if (!response.ok) {
        return {
          ok: false,
          name: teammate.name,
          message: response.error.message,
          retryable: response.error.code === 'RUN_ALREADY_ACTIVE',
          // Every slot taken, as against this teammate being mid-run.
          pool: response.error.code === 'RUN_ALREADY_ACTIVE' && response.error.busy === 'pool'
        }
      }
      await assignOwner(teammateId, response.data.missionId)
      return { ok: true, data: response.data }
    }


    /*
     * TAG TEAMMATES FROM ANY CONVERSATION (0.438, shared/tagging.ts): each
     * teammate tagged is started on their own route, in a conversation of
     * their own, and announced like a room member so the screen stays where
     * the person is. Busy is said, not queued.
     */
    /*
     * COMPARE (0.441, shared/compare.ts, docs/PLAN-2026-09-28-COMPARE.md): one
     * ask to two or three models, each column its own mission in the
     * teammate's conversation with only the route changed. Phase one compares
     * ANSWERS: every column runs in Ask, so it works in any folder and no two
     * columns can collide. Each column holds its own run slot, so one teammate
     * answers on several models at once.
     */
    const compareRefused = (message: string) => ({ ok: false, error: { code: 'COMPARE_REFUSED', message } }) as const
    /*
     * COMPARE CHANGES (0.445): in a git project each column can change its
     * own copy, cut from the person's last commit, under .locust/compare --
     * never among the teammates' own branches. Keep this one puts the kept
     * copy's changes into the folder, uncommitted; every copy is then removed.
     */
    /*
     * OUTSIDE THE FOLDER (0.493). The columns' worktrees were made under
     * `<folder>/.locust/compare/`, so a column's own path named the real
     * project around it -- and a free model in Auto, told to "work only in
     * this folder", edited the project it could see above instead of its own
     * copy (Sol's 0.491 pass: the original `index.html` changed before anything
     * was kept, and Keep then refused). Beside the plain copies, where no
     * path leads back.
     */
    const compareTrees = () => createWorktreeManager({ workspacePath, directory: compareRoot(), note: detail => note('worktree-add', detail) })
    const discardCompareTrees = async (compare: PublicCompare): Promise<void> => {
      if (compare.changes !== true || compare.changesIn !== undefined) return
      for (const column of compare.slots) await compareTrees().discard(compareTreeId(compare.compareId, column.slot)).catch(() => undefined)
    }
    const startCompareColumn = async (compare: PublicCompare, column: PublicCompareSlot, prompt: string, followUpOf?: string, retrying = false): Promise<string | undefined> => {
      const peer = compare.teammateId === undefined ? undefined : await peerContextFor(compare.teammateId)
      if (compare.teammateId !== undefined && peer === undefined) return 'That teammate is no longer on the team.'
      const cannot = compareRefusalOf(column.route.runtime, antigravityProbe.cliPath() !== undefined)
      if (cannot !== undefined) return cannot
      // A comparison that edits: the column's own copy of the project (0.445).
      let tree: string | undefined
      if (compare.changes === true && compare.changesIn === 'copy') {
        // Not a git project (0.448): a plain copy of the folder, which Keep writes back from.
        try {
          tree = await makeCompareCopy({ folder: workspacePath, compareId: compare.compareId, slot: column.slot })
        } catch (error) {
          return error instanceof Error ? error.message.replace('It answers in a copy of the folder, and this', 'It works in a copy of the folder, and this') : 'A copy of the folder could not be made for it.'
        }
      } else if (compare.changes === true && compare.changesIn === 'folder') {
        // Too big to copy (0.555): it works in the folder itself, as the bar says.
        tree = workspacePath
      } else if (compare.changes === true) {
        try {
          // From the commit the comparison started on (0.763): a run of it again starts there too.
          tree = await compareTrees().ensure({ teammateId: compareTreeId(compare.compareId, column.slot), name: `compare ${compare.compareId} ${column.slot}`, ...(compare.base === undefined ? {} : { from: compare.base }) })
        } catch (error) {
          return error instanceof Error ? error.message : 'A copy of the project could not be made for it.'
        }
      }
      // A column that cannot be held read-only answers in a copy, in the mode it can run (0.443).
      const inCopy = tree === undefined && compareNeedsCopy(column.route.runtime, process.platform)
      let copy: string | undefined = tree
      if (inCopy) {
        try {
          copy = await makeCompareCopy({ folder: workspacePath, compareId: compare.compareId, slot: column.slot })
        } catch (error) {
          return error instanceof Error ? error.message : 'A copy of the folder could not be made for it.'
        }
      }
      const response = await codexMissions.start(
        prompt,
        column.route.runtime as MissionRuntimeId,
        // Auto in its own copy when the person's Auto is on and the route asked for it (0.451); else Edit.
        copy === undefined
          ? 'ask'
          : compare.changes === true && column.route.mode === 'auto' && (await teammates.readSettings()).autoMode === true
            ? 'auto'
            : 'accept-edits',
        { ...(column.route.model === 'account-default' ? {} : { model: column.route.model }), ...(column.route.account === undefined ? {} : { account: column.route.account }), ...(column.route.effort === undefined ? {} : { effort: column.route.effort }) },
        sendToWindow,
        undefined,
        peer,
        followUpOf,
        undefined,
        undefined,
        undefined,
        { key: compareSlotKey(compare.teammateId, compare.compareId, column.slot), ...(copy === undefined ? {} : { cwd: copy }), ...(column.route.instructions === undefined ? {} : { instructions: column.route.instructions }), ...(column.route.skill === undefined ? {} : { skill: column.route.skill }) }
      )
      if (!response.ok) return response.error.code === 'RUN_ALREADY_ACTIVE' ? 'This column is still answering. Ask again when it has finished.' : response.error.message
      if (compare.rerunOf !== undefined) rerunRunIds.set(response.data.missionId, response.data.runId)
      if (compare.teammateId !== undefined) await assignOwner(compare.teammateId, response.data.missionId)
      await (retrying ? compares.retry(compare.compareId, column.slot, response.data.missionId) : compares.addTurn(compare.compareId, column.slot, response.data.missionId))
      sendToWindow({
        kind: 'mission-started',
        runId: response.data.runId,
        missionId: response.data.missionId,
        ...(compare.teammateId === undefined ? {} : { teammateId: compare.teammateId }),
        prompt,
        data: response.data,
        startedBy: { kind: 'compare', compareId: compare.compareId, slot: column.slot }
      })
      return undefined
    }
    /** Start every column that can take the ask; a column that cannot says why in its own place. */
    const askEveryColumn = async (compare: PublicCompare, prompt: string, followingUp: boolean) => {
      const refused: { slot: CompareSlotId; message: string }[] = []
      for (const column of compare.slots) {
        if (followingUp && (column.refused !== undefined || column.missionIds.length === 0)) continue
        const why = await startCompareColumn(compare, column, prompt, followingUp ? column.missionIds[column.missionIds.length - 1] : undefined).catch((error: unknown) =>
          error instanceof Error ? error.message : 'It could not be started.'
        )
        if (why === undefined) continue
        refused.push({ slot: column.slot, message: why })
        if (!followingUp) await compares.refuse(compare.compareId, column.slot, why).catch(() => undefined)
      }
      return { ok: true, data: { compare: (await compares.get(compare.compareId)) ?? compare, refused } } as const
    }
    ipcMain.handle(COMPARE_START_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return compareRefused('The request was rejected.')
      const noFolder = noWorkspaceRefusal()
      if (noFolder !== undefined) return compareRefused(noFolder.error.message)
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      const prompt = typeof input.prompt === 'string' ? input.prompt.trim() : ''
      // No teammate is needed to compare (Colin, 2026-09-28): absent is nobody's conversation.
      const teammateId = typeof input.teammateId === 'string' && input.teammateId.length > 0 ? input.teammateId : undefined
      const offered = (Array.isArray(input.routes) ? input.routes : [])
        .filter((route): route is Record<string, unknown> => typeof route === 'object' && route !== null)
      // What it knows, as a skill (0.811): one that will not read whole is refused in words, never dropped quietly.
      if (offered.some((route) => route.skill !== undefined && tuneSkillOf(route.skill) === undefined)) {
        return compareRefused('That skill could not be read whole: it needs a name of lower-case words joined by hyphens, one line on when to use it, and what it says. Nothing was started.')
      }
      const routes = offered
        .map((route) => ({
          runtime: typeof route.runtime === 'string' ? route.runtime : '',
          model: typeof route.model === 'string' && route.model.length > 0 ? route.model : 'account-default',
          ...(typeof route.account === 'string' ? { account: route.account } : {}), ...(typeof route.effort === 'string' ? { effort: route.effort } : {}),
          ...(typeof route.label === 'string' && route.label.trim().length > 0 ? { label: route.label.trim() } : {}),
          ...(route.mode === 'auto' ? { mode: 'auto' as const } : {}),
          ...(route.instructions === undefined ? {} : { instructions: route.instructions as string }),
          ...(typeof route.why === 'string' && route.why.trim().length > 0 ? { why: route.why.trim().slice(0, 400) } : {}),
          // Refused below unless it is a Claude Code tune column (compareSkillRefusal).
          ...(route.skill === undefined ? {} : { skill: tuneSkillOf(route.skill)! })
        }))
        .filter((route) => route.runtime.length > 0)
      const changes = input.changes === true
      const blind = input.blind === true
      // Blind: the person picked the models, so which column is which must be chance, not pick order.
      if (blind) {
        routes.splice(0, routes.length, ...shuffledColumns(routes, (upper) => randomInt(upper)))
      }
      const instructionRefusal = compareInstructionsRefusal(routes, teammateId)
      if (instructionRefusal !== undefined) return compareRefused(instructionRefusal)
      const skillRefusal = compareSkillRefusal(routes)
      if (skillRefusal !== undefined) return compareRefused(skillRefusal)
      if (prompt.length === 0) return compareRefused('There is nothing to compare yet. Write the ask first.')
      if (routes.length < MIN_COMPARE_SLOTS || routes.length > MAX_COMPARE_SLOTS) {
        return compareRefused(`Pick ${String(MIN_COMPARE_SLOTS)} or ${String(MAX_COMPARE_SLOTS)} different models to compare.`)
      }
      if (teammateId !== undefined && !(await teammates.list()).some((entry) => entry.teammateId === teammateId)) return compareRefused('That teammate is no longer on the team.')
      /*
       * A git project gives each column a worktree; any other folder, a plain
       * copy (0.448) -- and one too big to copy, the folder itself (0.555),
       * where 0.457 refused and sent the person back to Ask.
       */
      const changesIn = changes && !(await compareTrees().probe()).repository ? ((await copyRefusal(workspacePath)) === undefined ? ('copy' as const) : ('folder' as const)) : undefined
      // In a git project, the commit every column's copy is cut from (0.763): a run of it again starts there too.
      const base = changes && changesIn === undefined ? (await defaultRunGit(['rev-parse', 'HEAD'], workspacePath).catch(() => '')).trim() : ''
      try {
        return await askEveryColumn(await compares.create({ ...(teammateId === undefined ? {} : { teammateId }), prompt, routes, changes, blind, ...(changesIn === undefined ? {} : { changesIn }), ...(/^[0-9a-f]{40}$/.test(base) ? { base } : {}) }), prompt, false)
      } catch (error) {
        return compareRefused(error instanceof Error ? error.message : 'The comparison could not be started.')
      }
    })
    ipcMain.handle(COMPARE_ASK_CHANNEL, async (event, compareId: unknown, prompt: unknown) => {
      if (!fromOwnWindow(event)) return compareRefused('The request was rejected.')
      const compare = await compares.get(compareId).catch(() => undefined)
      if (compare === undefined) return compareRefused('That comparison is no longer here.')
      if (compare.kept !== undefined) return compareRefused('You kept one already; this conversation carries on with it.')
      if (compare.rerunOf !== undefined) return compareRefused('A run of your benchmark is a measurement: nothing in it is asked again, kept or added to. Use the comparison it runs again.')
      const text = typeof prompt === 'string' ? prompt.trim() : ''
      if (text.length === 0) return compareRefused('There is nothing to ask.')
      return askEveryColumn(compare, text, true)
    })
    ipcMain.handle(COMPARE_KEEP_CHANNEL, async (event, compareId: unknown, slot: unknown) => {
      if (!fromOwnWindow(event)) return compareRefused('The request was rejected.')
      if (typeof compareId !== 'string' || typeof slot !== 'string') return compareRefused('That column is not in a comparison.')
      try {
        const compare = await compares.get(compareId)
        if (compare === undefined) return compareRefused('That comparison is no longer here.')
        if (compare.rerunOf !== undefined) return compareRefused('A run of your benchmark is a measurement: nothing in it is asked again, kept or added to. Use the comparison it runs again.')
        let brought: readonly string[] | undefined
        if (compare.changes === true && compare.kept === undefined && compare.changesIn !== 'folder') {
          const result = await (compare.changesIn === 'copy'
            ? bringInCopy({ folder: workspacePath, compareId, slot: slot as CompareSlotId })
            : compareTrees().bringIn(compareTreeId(compareId, slot as CompareSlotId))
          ).catch((error: unknown) => ({ kind: 'does-not-apply' as const, message: error instanceof Error ? error.message : 'It did not say why.' }))
          if (result.kind === 'your-changes') {
            return compareRefused(`You have changed ${result.files.slice(0, 3).join(', ')}${result.files.length > 3 ? ` and ${String(result.files.length - 3)} more` : ''} yourself, and this answer changes ${result.files.length === 1 ? 'it' : 'them'} too, so nothing was put into your folder. Save or undo your change first, then keep it again.`)
          }
          if (result.kind === 'does-not-apply') {
            return compareRefused(`Its changes no longer fit your folder, which has changed since the comparison began, so nothing was put in. (${result.message.slice(0, 200)})`)
          }
          brought = result.kind === 'brought' ? result.files : []
        }
        const kept = await compares.keep(compareId, slot as CompareSlotId, brought)
        // The copies were for comparing; the conversation goes on in the folder. The worktrees go first, through
        // git, while their folders still say which branch each was on; then anything left of the folders (0.676).
        await discardCompareTrees(kept)
        await removeCompareCopies(compareId).catch(() => undefined)
        return { ok: true, data: { compare: kept, refused: [] } } as const
      } catch (error) {
        return compareRefused(error instanceof Error ? error.message : 'That column could not be kept.')
      }
    })
    /*
     * TRY AGAIN (0.444, Arena's per-column Regenerate): one column's newest
     * ask again, on the same model, following the same turn it followed. The
     * answer it replaces stays the comparison's, undrawn. The answer to a free
     * provider that was down, which a drive met on 2026-09-28.
     */
    ipcMain.handle(COMPARE_RETRY_CHANNEL, async (event, compareId: unknown, slot: unknown) => {
      if (!fromOwnWindow(event)) return compareRefused('The request was rejected.')
      const compare = await compares.get(compareId).catch(() => undefined)
      if (compare === undefined) return compareRefused('That comparison is no longer here.')
      if (compare.kept !== undefined) return compareRefused('You kept one already; this conversation carries on with it.')
      if (compare.rerunOf !== undefined) return compareRefused('A run of your benchmark is a measurement: nothing in it is asked again, kept or added to. Use the comparison it runs again.')
      const column = compare.slots.find((one) => one.slot === slot)
      if (column === undefined) return compareRefused('That comparison has no such column.')
      const newest = column.missionIds.at(-1)
      let prompt = compare.prompt
      if (newest !== undefined) {
        const recorded = await missionLedger.getMission(newest).catch(() => undefined)
        if (recorded === undefined) return compareRefused('Its last ask could not be read, so it cannot be tried again.')
        prompt = recorded.metadata.prompt
      }
      const why = await startCompareColumn(compare, column, prompt, column.missionIds.at(-2), true).catch((error: unknown) =>
        error instanceof Error ? error.message : 'It could not be started.'
      )
      if (why !== undefined) return compareRefused(why)
      return { ok: true, data: { compare: (await compares.get(compare.compareId)) ?? compare, refused: [] } } as const
    })
    /*
     * A JUDGE'S VIEW (0.520, main/compare-judge.ts), after Optima's judge
     * models. One run on the model the person picked, read-only, nobody's:
     * it reads each column's newest answer under its letter and says what it
     * would keep. The comparison records the run so it is never listed as a
     * conversation; the person still keeps one themselves.
     */
    /*
     * ANOTHER MODEL, THE SAME QUESTION (0.523), after Optima's "results on the
     * latest models as soon as they're released": a comparison of one question
     * takes one more column, asked what the others were asked. A decided
     * comparison keeps what it kept; the record counts the new one.
     */
    ipcMain.handle(COMPARE_ADD_MODEL_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return compareRefused('The request was rejected.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      const compare = await compares.get(input.compareId).catch(() => undefined)
      if (compare === undefined) return compareRefused('That comparison is no longer here.')
      if (compare.rerunOf !== undefined) return compareRefused('A run of your benchmark is a measurement: nothing in it is asked again, kept or added to. Use the comparison it runs again.')
      if (compare.changes === true) return compareRefused('A comparison that changes files cannot take another model after it starts. Start a new comparison instead.')
      if (compare.slots.some((column) => column.missionIds.length > 1)) return compareRefused('This comparison went on past its first question, so another model cannot be asked the same thing. Start a new comparison instead.')
      const route = (typeof input.route === 'object' && input.route !== null ? input.route : {}) as Record<string, unknown>
      if (!isMissionRuntime(route.runtime) || typeof route.model !== 'string' || route.model.length === 0 || route.model.length > 200) return compareRefused('Pick a model to ask.')
      let added: PublicCompare
      try {
        added = await compares.addColumn(compare.compareId, {
          runtime: route.runtime,
          model: route.model,
          ...(typeof route.account === 'string' ? { account: route.account } : {}), ...(typeof route.effort === 'string' ? { effort: route.effort } : {}),
          ...(typeof route.label === 'string' && route.label.trim().length > 0 ? { label: route.label.trim() } : {})
        })
      } catch (error) {
        return compareRefused(error instanceof Error ? error.message : 'That model could not be added. Nothing was asked.')
      }
      const column = added.slots.at(-1)
      if (column === undefined) return compareRefused('That model could not be added. Nothing was asked.')
      const why = await startCompareColumn(added, column, compare.prompt).catch((error: unknown) => (error instanceof Error ? error.message : 'It could not be started.'))
      if (why !== undefined) {
        await compares.refuse(compare.compareId, column.slot, why).catch(() => undefined)
        return { ok: true, data: { compare: (await compares.get(compare.compareId)) ?? added, refused: [{ slot: column.slot, message: why }] } } as const
      }
      return { ok: true, data: { compare: (await compares.get(compare.compareId)) ?? added, refused: [] } } as const
    })
    /**
     * A judge asked to read a comparison's answers (0.520), as the person asks one and as a run again does (0.763):
     * its run started and recorded as the comparison's, or why it was not.
     */
    const judgeCompare = async (
      compare: PublicCompare,
      route: CompareRoute,
      criteria: string | undefined,
      panel: readonly CompareRoute[] = []
    ): Promise<{ readonly ok: true; readonly compare: PublicCompare } | { readonly ok: false; readonly message: string }> => {
      const refused = (message: string) => ({ ok: false as const, message })
      for (const one of [route, ...panel]) {
        if (!isMissionRuntime(one.runtime) || one.model.length === 0 || one.model.length > 200) return refused('Pick a model to judge.')
        const cannot = compareRefusalOf(one.runtime, antigravityProbe.cliPath() !== undefined)
        if (cannot !== undefined) return refused(cannot.replace('a comparison', 'a judge'))
      }
      // Three judges voting (0.765): three models, on the points the person wrote -- a judge left to set its own would
      // score points the others never saw, and there would be nothing to vote on.
      if (panel.length > PANEL_JUDGES) return refused('Three judges at most.')
      if (new Set([route, ...panel].map((one) => `${one.runtime}:${one.model}`)).size !== panel.length + 1) return refused('Three judges are three different models: pick another for each.')
      if (panel.length > 0 && criteriaLines(criteria).length === 0) return refused('Three judges vote on the points you write: say what a good answer does, one point a line.')
      const answers: JudgedAnswer[] = []
      /*
       * JUDGES SEE THE CHECK (0.810, edit-check.ts): each column's copy ran the person's check after its turn, and a
       * judge waits for it -- first for the turn's slot, since the check is asked for just before that is released.
       */
      let checkCommand: string | undefined
      const checkOf = async (missionId: string, recorded: { readonly editChecks: readonly EditCheckResult[] }): Promise<JudgedCheck | undefined> => {
        const until = Date.now() + 30_000
        while (codexMissions.hasMission(missionId) && Date.now() < until) await new Promise((done) => setTimeout(done, 250))
        const ran = (await editCheck.afterTurn(missionId)) ?? recorded.editChecks.at(-1)
        if (ran === undefined) return undefined
        if (ran === 'running') return { outcome: 'running' }
        checkCommand ??= ran.command
        return { outcome: ran.outcome, ...(ran.newLines.length === 0 ? {} : { lines: ran.newLines }), ...(ran.unchanged ? { unchanged: true } : {}) }
      }
      for (const column of compare.slots) {
        const newest = column.missionIds.at(-1)
        if (newest === undefined) continue
        const recorded = await missionLedger.getMission(newest).catch(() => undefined)
        if (recorded === undefined || recorded.phase !== 'completed') continue
        const tracker = createTranscriptTracker()
        tracker.track(recorded.events)
        const said = tracker.latestFinal?.trim()
        if (said === undefined || said.length === 0) continue
        let diff: string | undefined
        let check: JudgedCheck | undefined
        if (compare.changes === true && compare.changesIn !== 'folder') {
          try {
            // THE COLUMN'S ACTUAL WORK (0.810): never its route or copy path in the material a judge reads.
            // Old worktrees lived under the project. Read that copy if present, never fall back to the person's HEAD.
            const root = compareRoot()
            const present = await stat(join(root, compareTreeId(compare.compareId, column.slot))).then(() => true, () => false)
            if (compare.changesIn !== 'copy' && compare.base === undefined) throw new Error('No recorded starting point.')
            diff = await copyDiff({
              folder: workspacePath, compareId: compare.compareId, slot: column.slot,
              root: present || compare.changesIn === 'copy' ? root : join(workspacePath, COMPARE_TREES_DIRECTORY),
              ...(compare.changesIn === 'copy' ? {} : { base: compare.base! }), runGit: defaultRunGit
            })
          } catch {
            return refused(`Answer ${column.slot.toUpperCase()}'s original changes could not be read. Its copy or starting files may no longer be here. Nothing was sent to a judge.`)
          }
          check = await checkOf(newest, recorded)
        }
        answers.push({ letter: column.slot.toUpperCase(), text: said, ...(diff === undefined ? {} : { diff }), ...(check === undefined ? {} : { check }) })
      }
      if (answers.length < 2) return refused('A judge needs at least two finished answers to read.')
      // Beside answers that were checked, one that was not says so, rather than reading as unchecked by omission.
      if (answers.some((answer) => answer.check !== undefined)) {
        for (const [index, answer] of answers.entries()) if (answer.check === undefined) answers[index] = { ...answer, check: { outcome: 'not-run' } }
        checkCommand ??= workspaceChosen ? (await teammates.readSettings()).checkCommands?.[workspaceIdFor(workspacePath)] : undefined
      }
      const newestAsk = compare.slots.flatMap((column) => column.missionIds.slice(-1))[0]
      const ask = newestAsk === undefined ? compare.prompt : ((await missionLedger.getMission(newestAsk).catch(() => undefined))?.metadata.prompt ?? compare.prompt)
      const runtime = route.runtime
      const model = route.model
      const asked = {
        ask,
        ...(criteria === undefined || criteria.length === 0 ? {} : { criteria }),
        ...(compare.changes === true ? { changes: compare.changesIn === 'folder' ? 'folder' as const : 'copies' as const } : {}),
        ...(checkCommand === undefined ? {} : { check: { command: checkCommand } }),
        // A tune (0.811): one model's setups, judged as such.
        ...(hasColumnInstructions(compare) ? { setups: true } : {})
      }
      let brief = judgePrompt({ ...asked, answers })
      // Too long for one message (0.553, 0.810): the whole answers AND their bounded diffs, filed under blind letters.
      if (brief.length > MAX_PROMPT_LENGTH) {
        try {
          await mkdir(join(workspacePath, ATTACHMENT_DIR), { recursive: true })
          const filed: JudgedAnswer[] = []
          const receipt = randomUUID()
          for (const answer of answers) {
            const file = `${ATTACHMENT_DIR}/judge-${receipt}-${answer.letter}.md`
            await writeFile(join(workspacePath, file), `${judgeAnswerText(answer, true)}\n`, 'utf8')
            filed.push({ ...answer, file })
          }
          await keepOutOfGit(workspacePath)
          brief = judgePrompt({ ...asked, answers: filed })
        } catch {
          return refused('The answers are too long to hand a judge, and they could not be written for it to read.')
        }
      }
      const startJudge = (one: CompareRoute, key: string) => codexMissions.start(
        brief,
        one.runtime as MissionRuntimeId,
        'ask',
        { ...(one.model === 'account-default' ? {} : { model: one.model }), ...(one.account === undefined ? {} : { account: one.account }), ...(typeof one.effort === 'string' ? { effort: one.effort } : {}) },
        sendToWindow,
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        { key }
      )
      const routeKept = (one: CompareRoute): CompareRoute => ({ runtime: one.runtime, model: one.model, ...(typeof one.account === 'string' ? { account: one.account } : {}), ...(typeof one.effort === 'string' ? { effort: one.effort } : {}), ...(typeof one.label === 'string' && one.label.trim().length > 0 ? { label: one.label.trim() } : {}) })
      const response = await startJudge({ ...route, runtime, model }, `judge:${compare.compareId}`)
      if (!response.ok) return refused(response.error.code === 'RUN_ALREADY_ACTIVE' ? 'The judge is still reading. Ask again when it has finished.' : response.error.message)
      const started = [response.data]
      // The panel, each on its own key so all three read at once; one that cannot start has no vote, and says so by
      // its absence from the panel -- the first judge's run is already going.
      const members: { route: CompareRoute; missionId: string }[] = []
      for (const [index, one] of panel.entries()) {
        const begun = await startJudge(one, `judge:${compare.compareId}:${String(index + 2)}`).catch(() => undefined)
        if (begun?.ok !== true) continue
        started.push(begun.data)
        members.push({ route: routeKept(one), missionId: begun.data.missionId })
      }
      if (compare.rerunOf !== undefined) for (const run of started) rerunRunIds.set(run.missionId, run.runId)
      const judged = await compares.judge(compare.compareId, routeKept({ ...route, runtime, model }), response.data.missionId, criteria, members)
      for (const run of started) {
        sendToWindow({
          kind: 'mission-started',
          runId: run.runId,
          missionId: run.missionId,
          prompt: 'Judge the answers',
          data: run,
          startedBy: { kind: 'judge', compareId: compare.compareId }
        })
      }
      return { ok: true, compare: judged }
    }
    ipcMain.handle(COMPARE_JUDGE_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return compareRefused('The request was rejected.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      const compare = await compares.get(input.compareId).catch(() => undefined)
      if (compare === undefined) return compareRefused('That comparison is no longer here.')
      if (compare.rerunOf !== undefined) return compareRefused('A run of your benchmark is a measurement: nothing in it is asked again, kept or added to. Use the comparison it runs again.')
      const route = (typeof input.route === 'object' && input.route !== null ? input.route : {}) as Record<string, unknown>
      if (!isMissionRuntime(route.runtime) || typeof route.model !== 'string' || route.model.length === 0 || route.model.length > 200) return compareRefused('Pick a model to judge.')
      const criteria = typeof input.criteria === 'string' ? input.criteria.trim().slice(0, MAX_JUDGE_CRITERIA) : undefined
      const panel: CompareRoute[] = []
      for (const member of Array.isArray(input.panel) ? input.panel.slice(0, PANEL_JUDGES + 1) : []) {
        const one = (typeof member === 'object' && member !== null ? member : {}) as Record<string, unknown>
        if (!isMissionRuntime(one.runtime) || typeof one.model !== 'string' || one.model.length === 0 || one.model.length > 200) return compareRefused('Pick a model for each judge.')
        panel.push({ runtime: one.runtime, model: one.model, ...(typeof one.account === 'string' ? { account: one.account } : {}), ...(typeof one.effort === 'string' ? { effort: one.effort } : {}), ...(typeof one.label === 'string' && one.label.trim().length > 0 ? { label: one.label.trim() } : {}) })
      }
      const judged = await judgeCompare(
        compare,
        {
          runtime: route.runtime,
          model: route.model,
          ...(typeof route.account === 'string' ? { account: route.account } : {}), ...(typeof route.effort === 'string' ? { effort: route.effort } : {}),
          ...(typeof route.label === 'string' && route.label.trim().length > 0 ? { label: route.label.trim() } : {})
        },
        criteria,
        panel
      )
      return judged.ok ? ({ ok: true, data: { compare: judged.compare, refused: [] } } as const) : compareRefused(judged.message)
    })
    /*
     * YOUR OWN BENCHMARK (0.763, compare-reruns.ts): a comparison the judge has scored, run again once or three times
     * more, unattended -- the same models, the same question, from the same commit, scored by the same judge on the
     * same points. The runs are the comparison's, never conversations of their own.
     */
    const rerunRunIds = new Map<string, string>()
    const compareReruns = createCompareReruns({
      compares,
      begin: async (compare) => {
        await askEveryColumn(compare, compare.prompt, false)
      },
      judge: async (compare, route, criteria, panel) => {
        const judged = await judgeCompare(compare, route, criteria, panel)
        return judged.ok ? undefined : judged.message
      },
      live: (missionId) => codexMissions.hasMission(missionId),
      phaseOf: async (missionId) => (await missionLedger.getMission(missionId).catch(() => undefined))?.phase,
      retry: async (compare, slot) => {
        const column = compare.slots.find((one) => one.slot === slot)
        return column === undefined ? 'That comparison has no such column.' : startCompareColumn(compare, column, compare.prompt, undefined, true)
      },
      stopMission: (missionId) => {
        const runId = rerunRunIds.get(missionId)
        if (runId !== undefined) codexMissions.cancel(runId)
      },
      tidy: async (compare) => {
        await discardCompareTrees(compare)
        await removeCompareCopies(compare.compareId).catch(() => undefined)
        for (const missionId of [...compare.slots.flatMap((column) => column.missionIds), ...(compare.judge?.missionIds ?? [])]) rerunRunIds.delete(missionId)
      },
      tell: (update) => sendToWindow({ kind: 'compare-reruns', compares: update.compares, progress: update.progress })
    })
    // A run again Locust closed on is over now (0.764): its chip said "waiting" for good.
    void compareReruns.settleLeftover().catch(() => undefined)
    ipcMain.handle(COMPARE_RERUN_CHANNEL, async (event, compareId: unknown, times: unknown, waitForChecks: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'The request was rejected.' }
      if (typeof compareId !== 'string' || typeof times !== 'number') return { ok: false, message: 'That comparison cannot be run again.' }
      const noFolder = noWorkspaceRefusal()
      if (noFolder !== undefined) return { ok: false, message: noFolder.error.message }
      if (waitForChecks !== undefined && typeof waitForChecks !== 'boolean') return { ok: false, message: 'That comparison cannot be run again.' }
      return compareReruns.start(compareId, times, waitForChecks === true)
    })
    ipcMain.handle(COMPARE_RERUN_CHECKED_CHANNEL, async (event, originId: unknown, compareId: unknown) => {
      return fromOwnWindow(event) && typeof originId === 'string' && typeof compareId === 'string' && compareReruns.checked(originId, compareId)
    })
    ipcMain.handle(COMPARE_RERUN_STOP_CHANNEL, async (event, compareId: unknown) => {
      if (fromOwnWindow(event) && typeof compareId === 'string') compareReruns.stop(compareId)
    })
    ipcMain.handle(COMPARE_RERUNS_CHANNEL, async (event) => (fromOwnWindow(event) ? compareReruns.progress() : []))
    /*
     * DRAFT THE POINTS (0.766, compare-score.ts pointsDraftRequest): one short read-only run on the model asked --
     * the first judge's -- that does not answer the request, only says what a good answer must do. Answered once it
     * is done, with the points to put in the box; the run is the comparison's, never a conversation.
     */
    ipcMain.handle(COMPARE_DRAFT_POINTS_CHANNEL, async (event, request: unknown) => {
      const no = (message: string) => ({ ok: false as const, message })
      if (!fromOwnWindow(event)) return no('The request was rejected.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      const compare = await compares.get(input.compareId).catch(() => undefined)
      if (compare === undefined) return no('That comparison is no longer here.')
      const route = (typeof input.route === 'object' && input.route !== null ? input.route : {}) as Record<string, unknown>
      if (!isMissionRuntime(route.runtime) || typeof route.model !== 'string' || route.model.length === 0 || route.model.length > 200) return no('Pick a model to draft them.')
      const cannot = compareRefusalOf(route.runtime, antigravityProbe.cliPath() !== undefined)
      if (cannot !== undefined) return no(cannot.replace('a comparison', 'the points'))
      const model = route.model
      const response = await codexMissions.start(
        pointsDraftRequest(compare.prompt),
        route.runtime,
        'ask',
        { ...(model === 'account-default' ? {} : { model }), ...(typeof route.account === 'string' ? { account: route.account } : {}), ...(typeof route.effort === 'string' ? { effort: route.effort } : {}) },
        sendToWindow,
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        { key: `draft:${compare.compareId}` }
      )
      if (!response.ok) return no(response.error.code === 'RUN_ALREADY_ACTIVE' ? 'The points are being drafted already.' : response.error.message)
      const { missionId, runId } = response.data
      // Kept before it is drawn anywhere, so the window never lists it among the conversations.
      const held = await compares.addPointDraft(compare.compareId, missionId).catch(() => undefined)
      if (held !== undefined) sendToWindow({ kind: 'compare-reruns', compares: [held], progress: compareReruns.progress() })
      sendToWindow({ kind: 'mission-started', runId, missionId, prompt: 'Draft the points', data: response.data, startedBy: { kind: 'judge', compareId: compare.compareId } })
      for (let waited = 0; waited < 240 && codexMissions.hasMission(missionId); waited += 1) await new Promise((done) => setTimeout(done, 1_000))
      if (codexMissions.hasMission(missionId)) {
        codexMissions.cancel(runId)
        return no('It took too long to draft them. Write the points yourself, or pick another model.')
      }
      const recorded = await missionLedger.getMission(missionId).catch(() => undefined)
      const tracker = createTranscriptTracker()
      tracker.track(recorded?.events ?? [])
      const points = draftedPoints(tracker.latestFinal ?? '')
      return points.length === 0 ? no('It wrote no points. Write them yourself, or pick another model.') : { ok: true as const, points }
    })
    // Nothing refuses a comparison that edits now (0.555): a folder too big to copy works in place.
    ipcMain.handle(COMPARE_CHANGES_REFUSAL_CHANNEL, () => undefined)
    // ...but said before it starts (0.675): Auto's line promised each model its own copy where there would be none.
    ipcMain.handle(COMPARE_IN_PLACE_CHANNEL, async (event) =>
      fromOwnWindow(event) && !(await compareTrees().probe()).repository && (await copyRefusal(workspacePath)) !== undefined
    )
    ipcMain.handle(COMPARE_CHANGES_CHANNEL, async (event, compareId: unknown) => {
      if (!fromOwnWindow(event)) return compareRefused('The request was rejected.')
      const compare = await compares.get(compareId).catch(() => undefined)
      if (compare === undefined) return compareRefused('That comparison is no longer here.')
      const columns: Partial<Record<CompareSlotId, { files: number; added?: number; removed?: number }>> = {}
      // In the folder itself (0.555) the columns' changes are mixed in one place: no count it can stand behind.
      if (compare.changes === true && compare.kept === undefined && compare.changesIn !== 'folder') {
        for (const column of compare.slots) {
          if (column.missionIds.length === 0) continue
          try {
            columns[column.slot] = compare.changesIn === 'copy'
              ? await copyLineChanges({ folder: workspacePath, compareId: compare.compareId, slot: column.slot, runGit: defaultRunGit })
              : await compareTrees().changes(compareTreeId(compare.compareId, column.slot))
          } catch {
            // A copy that cannot be read says nothing, rather than a number it cannot stand behind.
          }
        }
      }
      return { ok: true, data: { columns } } as const
    })
    ipcMain.handle(COMPARE_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return compareRefused('The request was rejected.')
      try {
        return { ok: true, data: { compares: await compares.list() } } as const
      } catch (error) {
        return compareRefused(error instanceof Error ? error.message : 'The comparisons could not be read.')
      }
    })

    /*
     * SCOUT, A TEAMMATE THAT KEEPS WATCH (0.756, scout.ts). Its looks run as a comparison's judge does: one turn in Ask
     * mode on Scout's own route, never listed as a conversation (the window knows them by the state's lookIds), whose
     * answer fills in the cards Locust has already made in its own words. GitHub is read with the person's own CLI.
     */
    const scoutGh = (args: readonly string[], cwd: string): Promise<string> => new Promise((resolve, reject) => {
      execFile('gh', [...args], {
        cwd, windowsHide: true, timeout: 30_000, maxBuffer: 4 * 1024 * 1024,
        env: { ...process.env, GH_PROMPT_DISABLED: '1', GH_NO_UPDATE_NOTIFIER: '1' }
      }, (error, stdout) => { if (error) reject(error); else resolve(String(stdout)) })
    })
    /*
     * A look's events go to the window like any run's; one saying its provider will not answer while the runtime
     * keeps trying (the same notice the thread offers another model beside, missionView RETRYING_NOTE) stops the
     * look and moves it to the next free model, rather than leaving it to run out of time.
     */
    const SCOUT_RETRYING = /^opencode\.runtime_error$|\.provider_busy\.runtime_error$/
    const scoutEmit = (update: CodexMissionUpdate): void => {
      sendToWindow(update)
      if (update.kind === 'event' && update.event.type === 'adapter.diagnostic' && SCOUT_RETRYING.test(update.event.payload.code)) {
        void keptScout.providerRefused(update.missionId, update.event.payload.message.replace(/,? and OpenCode is trying again on its own\.(?: To go on now, press Stop and pick another model\.)?$/, '.'))
      }
    }
    const scoutNameOf = async (teammateId: string | undefined): Promise<string | undefined> =>
      teammateId === undefined ? undefined : (await teammates.list().catch(() => [])).find((mate) => mate.teammateId === teammateId)?.name
    const readComments = createCommentReader(scoutGh)
    /*
     * SCOUT PREPARES THE FIX (0.828, scout-fix.ts): in a copy under ~/.locust/scout-fixes, on the picked teammate's own
     * route, in Edit. Its own slot, so it runs beside whatever that teammate is doing, and never commits their branch.
     */
    const fixes = createScoutFixes({
      runCheck: async (command, cwd) => {
        const run = await runCheckCommand(command, cwd)
        const lines = run.error !== undefined
          ? [run.error]
          : run.timedOut
            ? ['The check ran past its time limit and was stopped.']
            : comparableLines(run.output).slice(-25).map((line) => (line.length > 300 ? `${line.slice(0, 299)}…` : line))
        return { passed: run.error === undefined && !run.timedOut && run.exitCode === 0, lines }
      },
      start: async ({ fixId, teammateId, prompt, cwd, followUpOf }) => {
        if (!workspaceChosen) return { refused: NO_WORKSPACE_MESSAGE }
        const mate = (await teammates.list()).find((one) => one.teammateId === teammateId)
        if (mate?.route === undefined) return { refused: `${mate?.name ?? 'That teammate'} has no model yet. Pick one on the Team screen.` }
        const route = mate.route
        const peer = await peerContextFor(teammateId)
        const response = await codexMissions.start(
          prompt,
          route.runtime,
          'accept-edits',
          { ...(route.model === 'account-default' ? {} : { model: route.model }), ...(route.account === undefined ? {} : { account: route.account }), ...(typeof route.effort === 'string' ? { effort: route.effort } : {}) },
          sendToWindow,
          undefined,
          peer,
          followUpOf,
          undefined,
          undefined,
          undefined,
          { key: `scout-fix:${fixId}`, cwd }
        )
        if (!response.ok) return { refused: response.error.message }
        await assignOwner(teammateId, response.data.missionId)
        sendToWindow({ kind: 'mission-started', runId: response.data.runId, missionId: response.data.missionId, teammateId, prompt, data: response.data })
        return { missionId: response.data.missionId, runId: response.data.runId }
      },
      stop: (runId) => { codexMissions.cancel(runId) },
      endOf: async (missionId) => {
        const recorded = await missionLedger.getMission(missionId).catch(() => undefined)
        if (recorded === undefined) return { phase: 'other' }
        const tracker = createTranscriptTracker()
        tracker.track(recorded.events)
        const said = tracker.latestFinal?.trim()
        const failedEvent = [...recorded.events].reverse().find((event) => event.type === 'run.failed')
        const failed = failedEvent?.type === 'run.failed' ? failedEvent.payload.message : recorded.hostFailures.at(-1)?.message
        const phase = recorded.phase === 'completed' || recorded.phase === 'failed' || recorded.phase === 'cancelled' ? recorded.phase : 'other'
        return {
          phase,
          ...(said === undefined || said.length === 0 ? {} : { said }),
          ...(failed === undefined ? {} : { failed }),
          ...(said !== undefined && parseDecision(said) !== undefined ? { asked: true } : {})
        }
      }
    })
    scoutFixes = fixes
    /** gh with `input` on its stdin (0.829): the away look's secret is never put on a command line. */
    const scoutGhInput = (args: readonly string[], cwd: string, input: string): Promise<string> => new Promise((resolve, reject) => {
      const child = execFile('gh', [...args], {
        cwd, windowsHide: true, timeout: 60_000, maxBuffer: 4 * 1024 * 1024,
        env: { ...process.env, GH_PROMPT_DISABLED: '1', GH_NO_UPDATE_NOTIFIER: '1' }
      }, (error, stdout) => { if (error) reject(error); else resolve(String(stdout)) })
      child.stdin?.end(input)
    })
    /** The phones' way from anywhere, once it is made below: the away look leaves its letters in its room. */
    let scoutAwayRelay: ScoutAwayRelay | undefined
    /*
     * SCOUT WHILE YOUR COMPUTER IS OFF (0.829, scout-away.ts): a workflow in the folder's repository looks for Scout
     * with the PC off and leaves letters at the relay; this reads them when the PC is back.
     */
    const scoutAway = createScoutAway({
      file: join(app.getPath('userData'), SCOUT_AWAY_FILE),
      folder: () => (workspaceChosen ? workspacePath : undefined),
      gh: scoutGh,
      ghInput: scoutGhInput,
      relay: () => scoutAwayRelay,
      scout: {
        told: () => keptScout.told(),
        absorb: async (letters) => { await keptScout.absorb(letters) },
        hours: () => keptScout.hours()
      },
      script: () => readFile(app.isPackaged ? join(process.resourcesPath, 'scout-away.mjs') : join(__dirname, '../../resources/scout-away.mjs'), 'utf8'),
      changed: (status) => {
        for (const window of BrowserWindow.getAllWindows()) {
          if (!window.isDestroyed() && !window.webContents.isDestroyed()) window.webContents.send(SCOUT_AWAY_CHANGED, status)
        }
      }
    })
    const keptScout = createScout({
      catchUp: () => scoutAway.catchUp(),
      comments: async (watch, team, at) => workspaceChosen ? readComments(workspacePath, watch, team, at) : { watch, facts: [] },
      lastTeammate: async () => {
        const owners = await teammates.missionOwners()
        const files = await missionLedger.missionFiles?.()
        // The newest 30 owned conversations at most: each is read whole, and with many elsewhere this read every one.
        const recent = [...(files?.files ?? [])].sort((a, b) => b.modifiedAt - a.modifiedAt).filter((file) => owners[file.missionId] !== undefined).slice(0, 30)
        for (const file of recent) {
          const mission = await missionLedger.getMission(file.missionId).catch(() => undefined)
          if (mission?.metadata.workspaceId === memoryWorkspaceId) return owners[file.missionId]
        }
        return undefined
      },
      postComment: async (repository, number, text) => {
        if (!workspaceChosen) throw new Error(NO_WORKSPACE_MESSAGE)
        return postComment((args, cwd, input) => new Promise((resolve, reject) => {
          const child = execFile('gh', [...args], { cwd, windowsHide: true, timeout: 30_000, maxBuffer: 4 * 1024 * 1024, env: { ...process.env, GH_PROMPT_DISABLED: '1', GH_NO_UPDATE_NOTIFIER: '1' } }, (error, stdout) => error ? reject(error) : resolve(String(stdout)))
          child.stdin?.on('error', () => { /* execFile reports failure */ })
          child.stdin?.end(input)
        }), workspacePath, repository, number, text)
      },
      commentAnswer: async (missionId) => {
        const recorded = await missionLedger.getMission(missionId).catch(() => undefined)
        if (recorded === undefined || !['completed', 'failed', 'cancelled'].includes(recorded.phase)) return undefined
        const tracker = createTranscriptTracker()
        tracker.track(recorded.events)
        if (recorded.phase === 'completed' && tracker.latestFinal?.trim()) return { text: tracker.latestFinal }
        const failed = [...recorded.events].reverse().find((event) => event.type === 'run.failed')
        return { failed: failed?.type === 'run.failed' ? failed.payload.message : recorded.hostFailures.at(-1)?.message ?? 'The turn ended without an answer.' }
      },
      file: join(app.getPath('userData'), 'scout.json'),
      logFile: join(app.getPath('userData'), 'scout-looks.json'),
      fixes: {
        prepare: async (input) => {
          if (!workspaceChosen) return { kind: 'unfixed', why: NO_WORKSPACE_MESSAGE }
          return fixes.prepare({ ...input, folder: workspacePath })
        },
        apply: (fixId) => fixes.apply(fixId),
        discard: (fixId) => fixes.discard(fixId),
        stop: (why) => { fixes.stop(why) }
      },
      github: async () => {
        if (!workspaceChosen) return undefined
        let why: ScoutGithubUnread | undefined
        return (await readScoutGithub(workspacePath, scoutGh, '@me', (reason) => { why = reason })) ?? why
      },
      waiting: async () => Promise.all(raisedApprovals.waiting().map(async ({ request, teammateId }) => {
        const name = await scoutNameOf(teammateId)
        return {
          approvalId: request.approvalId, missionId: request.missionId, summary: request.summary, requestedAt: request.requestedAt,
          ...(teammateId === undefined ? {} : { teammateId }), ...(name === undefined ? {} : { name })
        }
      })),
      failedTurn: async (missionId) => {
        const recorded = await missionLedger.getMission(missionId).catch(() => undefined)
        if (recorded === undefined || recorded.phase !== 'failed') return undefined
        const owner = (await teammates.missionOwners().catch(() => ({}) as Record<string, string>))[missionId]
        const name = await scoutNameOf(owner)
        // The runtime's own words for why, as the thread shows them (the first drive's Scout guessed without them).
        const failedEvent = [...recorded.events].reverse().find((event) => event.type === 'run.failed')
        const said = failedEvent?.type === 'run.failed' ? failedEvent.payload.message : undefined
        const why = said ?? recorded.hostFailures.at(-1)?.message
        return {
          missionId, asked: recorded.metadata.prompt,
          ...(owner === undefined ? {} : { teammateId: owner }), ...(name === undefined ? {} : { name }), ...(why === undefined ? {} : { why })
        }
      },
      continuesFrom: async (missionId) => (await missionLedger.getMission(missionId).catch(() => undefined))?.metadata.continuesFrom?.missionId,
      team: async () => (await teammates.list()).map((mate) => ({ teammateId: mate.teammateId, name: mate.name, role: mate.roleTitle ?? mate.role })),
      ask: async (teammateId, prompt, model) => {
        if (!workspaceChosen) return { refused: NO_WORKSPACE_MESSAGE }
        const route = (await teammates.list()).find((mate) => mate.teammateId === teammateId)?.route
        if (route === undefined) return { refused: 'Scout has no model to read with. Pick one for it on the Team screen.' }
        // A free model in Scout's own one's place, when that one's provider would not answer (scout.ts gaveUp).
        const runtime = model === undefined ? route.runtime : 'opencode'
        const chosen = model ?? route.model
        const response = await codexMissions.start(
          prompt,
          runtime,
          'ask',
          { ...(chosen === 'account-default' ? {} : { model: chosen }), ...(model === undefined && route.account !== undefined ? { account: route.account } : {}), ...(model === undefined && typeof route.effort === 'string' ? { effort: route.effort } : {}) },
          scoutEmit,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          // One look at a time on each model: a look moved to the next free model starts while the stopped one ends.
          // Sealed (0.824): the look reads only what Scout gives it, which holds other people's words; no tool, no memory brief.
          { key: `scout:${teammateId}:${chosen}`, sealed: true }
        )
        if (!response.ok) return { refused: response.error.code === 'RUN_ALREADY_ACTIVE' ? 'Scout is still reading its last look.' : response.error.message }
        sendToWindow({ kind: 'mission-started', runId: response.data.runId, missionId: response.data.missionId, prompt: 'Scout looked', data: response.data, startedBy: { kind: 'scout' } })
        return { missionId: response.data.missionId, runId: response.data.runId }
      },
      stop: (runId) => { codexMissions.cancel(runId) },
      // Along the free models, best first, for a Scout on a free model: never onto a paid one.
      nextFreeModel: async (teammateId, tried) => {
        const route = (await teammates.list()).find((mate) => mate.teammateId === teammateId)?.route
        if (route === undefined || !isFreeRoute(route.runtime, route.model)) return undefined
        return FREE_MODELS_BEST_FIRST.find((id) => id !== route.model && !tried.includes(id))
      },
      answerOf: async (missionId) => {
        const recorded = await missionLedger.getMission(missionId).catch(() => undefined)
        if (recorded === undefined) return { failed: 'Scout’s look left no record to read.' }
        const tracker = createTranscriptTracker()
        tracker.track(recorded.events)
        const text = tracker.latestFinal?.trim()
        if (recorded.phase === 'completed' && text !== undefined && text.length > 0) return { text }
        const why = recorded.hostFailures.at(-1)?.message
        return { failed: recorded.phase === 'completed' ? 'Scout’s model said nothing.' : `Scout’s model did not answer${why === undefined ? '.' : `: ${why.slice(0, 200)}`}` }
      },
      makeTeammate: async (route) => {
        const roster = await teammates.list()
        // The Scout this Locust made before, if Scout's own file was lost: never a second one beside it.
        const before = roster.find((mate) => mate.role === 'Custom' && mate.roleTitle === 'Keeps watch')
        if (before !== undefined) return before.teammateId
        const taken = new Set(roster.map((mate) => mate.name.toLowerCase()))
        const name = ['Scout', 'Scout 2', 'Scout 3', 'Lookout'].find((one) => !taken.has(one.toLowerCase())) ?? `Scout ${String(roster.length + 1)}`
        const made = await teammates.create({ name, hue: 'teal', role: 'Custom', roleTitle: 'Keeps watch' })
        await teammates.rememberRoute(made.teammateId, { runtime: route.runtime, model: route.model, ...(route.account === undefined ? {} : { account: route.account }), mode: 'ask' })
        return made.teammateId
      },
      teammateExists: async (teammateId) => (await teammates.list()).some((mate) => mate.teammateId === teammateId),
      // The folder's check (0.815, scout-folder.ts): only when the folder moved since the last look, never mid-turn.
      folderMark: async () => (workspaceChosen ? folderMarkOf(workspacePath) : undefined),
      runCheck: async () => {
        const ran = await runFolderCheck(workspacePath)
        return ran === undefined ? undefined : { command: ran.command, passed: ran.passed, lines: ran.tail }
      },
      folderBusy: () => codexMissions.liveMissionIds().length + antigravityMissions.liveMissionIds().length > 0,
      // A turn the person sent from the chat box, still running: a look on Scout's timer waits for it (0.815).
      personBusy: () => [...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()].some((missionId) => sentByPerson.has(missionId)),
      // The morning brief's count (0.816): turns whose record changed since, read newest first, Scout's own looks left out.
      briefFacts: async (since, looks) => {
        const from = Date.parse(since)
        const own = new Set(looks)
        const listing = await missionLedger.missionFiles?.().catch(() => undefined)
        let finished = 0
        let failed = 0
        // The 80 newest, and said as "at least" past them (0.823, Grok's review): the cut was silent, and not the newest.
        const ended = (listing?.files ?? []).filter((one) => one.modifiedAt >= from && !own.has(one.missionId)).sort((a, b) => b.modifiedAt - a.modifiedAt)
        for (const file of ended.slice(0, 80)) {
          const mission = (await missionLedger.readMission?.(file.missionId).catch(() => undefined))?.mission
          if (mission === undefined) continue
          if (mission.phase === 'completed') finished += 1
          else if (mission.phase === 'failed') failed += 1
        }
        return { finished, failed, ...(ended.length > 80 ? { more: true } : {}) }
      },
      changed: (state) => {
        scoutQuiet = state.settings.quiet
        scoutAway.told()
        for (const window of BrowserWindow.getAllWindows()) {
          if (!window.isDestroyed() && !window.webContents.isDestroyed()) window.webContents.send(SCOUT_CHANGED, state)
        }
      },
      notify: (card) => { attention.runFinished({ title: 'Scout', body: card.said, missionId: card.open?.kind === 'conversation' ? card.open.missionId : undefined }) },
      raised: (cards) => phoneAlerts.scoutCards(cards),
      heldAlerts: (since) => phoneAlerts.heldSince(since)
    })
    scout = keptScout
    void keptScout.state().then((state) => { scoutQuiet = state.settings.quiet }, () => undefined)
    ipcMain.handle(SCOUT_STATE, (event) => (fromOwnWindow(event) ? keptScout.state() : undefined))
    ipcMain.handle(SCOUT_TURN_ON, async (event, route: unknown) => {
      if (!fromOwnWindow(event)) return { refused: 'That request was rejected.' }
      const asked = (typeof route === 'object' && route !== null ? route : {}) as Record<string, unknown>
      const kept = { runtime: asked.runtime, model: asked.model, ...(typeof asked.account === 'string' ? { account: asked.account } : {}), mode: 'ask' }
      // A Scout already on the team needs no route: it reads with its own.
      return keptScout.turnOn(isTeammateRoute(kept) ? { runtime: kept.runtime, model: kept.model, ...(kept.account === undefined ? {} : { account: kept.account }) } : undefined)
    })
    ipcMain.handle(SCOUT_SET, async (event, change: unknown) => {
      if (!fromOwnWindow(event)) return keptScout.state()
      const before = await keptScout.hours()
      const state = await keptScout.set(change)
      // The away look keeps Scout's hours: changed here, its secret says them again.
      if (JSON.stringify(await keptScout.hours()) !== JSON.stringify(before)) scoutAway.hoursChanged()
      return state
    })
    ipcMain.handle(SCOUT_AWAY_STATUS, (event, check: unknown) => (fromOwnWindow(event) ? scoutAway.status(check === true) : { state: 'off' }))
    ipcMain.handle(SCOUT_AWAY_SET, (event, on: unknown) => (fromOwnWindow(event) ? scoutAway.set(on) : scoutAway.status()))
    ipcMain.handle(SCOUT_DISMISS, (event, cardId: unknown, how: unknown) => (fromOwnWindow(event) ? keptScout.dismiss(cardId, how) : keptScout.state()))
    ipcMain.handle(SCOUT_POST, (event, cardId: unknown) => fromOwnWindow(event) ? keptScout.post(cardId) : keptScout.state())
    ipcMain.handle(SCOUT_EDIT_POST, (event, cardId: unknown, text: unknown) => fromOwnWindow(event) ? keptScout.editPost(cardId, text) : keptScout.state())
    ipcMain.handle(SCOUT_LOOK, (event) => (fromOwnWindow(event) ? keptScout.look() : keptScout.state()))
    ipcMain.handle(SCOUT_NOTES, (event, text: unknown) => (fromOwnWindow(event) ? keptScout.saveNotes(text) : keptScout.state()))
    ipcMain.handle(SCOUT_TUNE, (event, cardId: unknown) => (fromOwnWindow(event) ? keptScout.tune(cardId) : keptScout.state()))
    ipcMain.handle(SCOUT_LOG, (event) => (fromOwnWindow(event) ? keptScout.log() : { entries: [], held: [] }))
    ipcMain.handle(SCOUT_FIX_APPLY, (event, cardId: unknown) => (fromOwnWindow(event) ? keptScout.applyFix(cardId) : keptScout.state()))
    ipcMain.handle(SCOUT_FIX_SKIP, (event, cardId: unknown) => (fromOwnWindow(event) ? keptScout.skipFix(cardId) : keptScout.state()))
    ipcMain.handle(SCOUT_FIX_AGAIN, (event, cardId: unknown) => (fromOwnWindow(event) ? keptScout.fixAgain(cardId) : keptScout.state()))
    void keptScout.start()
    app.once('will-quit', () => keptScout.dispose())

    ipcMain.handle(TEAMMATES_TAG_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return { ok: false, message: 'That request was rejected.' } as const
      const refused = noWorkspaceRefusal()
      if (refused !== undefined) return { ok: false, message: refused.error.message } as const
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      const message = typeof input.message === 'string' ? input.message : ''
      const ids = Array.isArray(input.teammateIds) ? [...new Set(input.teammateIds.filter((id): id is string => typeof id === 'string'))].slice(0, MAX_TAGGED) : []
      if (message.trim().length === 0 || ids.length === 0) return { ok: false, message: 'Nobody was tagged, or there was nothing to send.' } as const
      const roster = await teammates.list()
      const fromName = typeof input.fromTeammateId === 'string' ? roster.find((entry) => entry.teammateId === input.fromTeammateId)?.name : undefined
      let answer: string | undefined
      if (typeof input.fromMissionId === 'string') {
        const recovered = await missionLedger.getMission(input.fromMissionId).catch(() => undefined)
        if (recovered !== undefined) {
          const tracker = createTranscriptTracker()
          tracker.track(recovered.events)
          answer = tracker.latestFinal
        }
      }
      const started: string[] = []
      const notStarted: { name: string; message: string }[] = []
      for (const teammateId of ids) {
        const teammate = roster.find((entry) => entry.teammateId === teammateId)
        const peer = await peerContextFor(teammateId)
        if (teammate === undefined || peer === undefined) {
          notStarted.push({ name: teammate?.name ?? teammateId, message: 'No longer on the team.' })
          continue
        }
        const route = teammate.route ?? { runtime: 'codex' as const, model: 'account-default', mode: 'ask' as const }
        if (route.runtime === 'antigravity' && antigravityProbe.cliPath() === undefined) {
          notStarted.push({ name: teammate.name, message: 'Antigravity can be tagged once Antigravity CLI is installed.' })
          continue
        }
        const prompt = taggedPrompt({ message, ...(fromName === undefined ? {} : { fromName }), ...(answer === undefined ? {} : { answer }) })
        const response = await codexMissions.start(
          prompt,
          route.runtime,
          route.mode,
          { ...(route.model === 'account-default' ? {} : { model: route.model }), ...(route.account === undefined ? {} : { account: route.account }), ...(route.effort === undefined ? {} : { effort: route.effort }) },
          sendToWindow,
          undefined,
          peer
        )
        if (!response.ok) {
          notStarted.push({ name: teammate.name, message: response.error.code === 'RUN_ALREADY_ACTIVE' ? `${teammate.name} is busy. Tag them again when they are free.` : response.error.message })
          continue
        }
        await assignOwner(teammateId, response.data.missionId)
        sendToWindow({ kind: 'mission-started', runId: response.data.runId, missionId: response.data.missionId, teammateId, prompt: message, data: response.data, startedBy: { kind: 'tag' } })
        started.push(teammate.name)
      }
      return { ok: true, started, refused: notStarted } as const
    })

    ipcMain.handle(ROOM_POST_CHANNEL, async (event, request: unknown) => {
      const owner = BrowserWindow.fromWebContents(event.sender)
      if (owner === null || !fromOwnWindow(event)) return roomRejected('The post could not be made.')
      const refused = noWorkspaceRefusal()
      if (refused !== undefined) return roomRejected(refused.error.message)
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      const roomId = typeof input.roomId === 'string' ? input.roomId : undefined
      const text = typeof input.text === 'string' ? input.text : ''
      if (roomId === undefined || text.trim().length === 0) return roomRejected('Write something to post.')
      let room: Awaited<ReturnType<typeof rooms.get>>
      try {
        room = await rooms.get(roomId)
      } catch {
        return roomRejected('The rooms file could not be read, so nothing was posted.')
      }
      if (room === undefined) return roomRejected('That room no longer exists.')
      // 0.371: the members the person named, or everyone (`recipientsOf`).
      const recipients = recipientsOf(room.teammateIds, input.to)
      if (!recipients.ok) return roomRejected(recipients.message)

      // One run per teammate, each on THEIR route. A teammate with no route
      // of their own yet -- never started by a person -- runs on Codex's
      // account default in read-only, the same as a fresh teammate would.
      const refusals: { teammateId: string; name: string; message: string }[] = []
      let roster: Awaited<ReturnType<typeof teammates.list>>
      try {
        roster = await teammates.list()
      } catch {
        return roomRejected('The teammate roster could not be read, so nothing was posted.')
      }

      /*
       * THE POST EXISTS BEFORE ANYONE IS ASKED.
       *
       * It used to be written at the END: start everyone in sequence, then
       * record the post, then announce every run at once. So a person
       * watched an empty room while work was already underway -- Astra
       * measured a process running 45,292 ms before its own row appeared,
       * and it is the cause of every "cards: 0" the room drives reported
       * immediately after posting, which I had seen and not explained.
       *
       * Now everyone starts QUEUED, and each member leaves the queue the
       * moment their run begins -- announced there and then. Which is
       * exactly what `room-tasks` does when a slot frees, so the two paths
       * are one path, and a post that cannot reach everyone at once is the
       * same thing as a post that reaches them slowly.
       */
      let post: RoomPost
      try {
        post = await rooms.addPost(roomId, {
          text,
          missions: {},
          queued: [...recipients.asked],
          ...(recipients.to === undefined ? {} : { to: recipients.to })
        })
      } catch (error) {
        return roomRejected(error instanceof Error ? error.message : 'The post could not be recorded.')
      }
      // Said the moment it is on disk. Everything below this line can take as
      // long as a cold runtime needs; the person's own words are already on
      // screen where they put them.
      sendToWindow({ kind: 'room-posted', roomId, postId: post.postId })

      for (const teammateId of recipients.asked) {
        const attempt = await startRoomMember(room, teammateId, post, roster)
        if (!attempt.ok) {
          /*
           * "Not now" is not a refusal.
           *
           * `attempt.retryable` marks the two cases waiting fixes -- the
           * live cap is full, or that teammate is already working. Those
           * members wait in the post's queue and are started by
           * `room-tasks` the moment a slot frees. Everything else -- gone
           * from the roster, a runtime a room cannot post to -- is recorded
           * as refused, because waiting will never fix it.
           */
          if (attempt.retryable) continue
          refusals.push({ teammateId, name: attempt.name, message: attempt.message })
          await rooms.refuseQueued(roomId, post.postId, teammateId, attempt.message).catch(() => undefined)
          continue
        }
        await rooms.startQueued(roomId, post.postId, teammateId, attempt.data.missionId).catch(() => undefined)
        /*
         * Claim the row the person named, at START -- on THIS path.
         *
         * `room-tasks` has done this since 0.116.0, but only where a queued
         * member is drained after some other run ends. Grok measured the
         * hole on the second pass (2026-09-14, finding 1): the post that
         * actually starts the named teammate comes through here, so the one
         * member the person pointed at was the one member the claim never
         * saw. The row sat unassigned through the whole live run and was
         * claimed only by the reply's own end-of-text block, which is what
         * 0.116.0 already had.
         *
         * Read fresh: members ahead of this one in the loop may have moved
         * the board since the post was made.
         */
        const current = await rooms.get(roomId).catch(() => undefined)
        const named =
          current === undefined
            ? undefined
            : rowToClaimAtStart({
                postText: text,
                tasks: current.tasks,
                members: roster.filter((mate) => current.teammateIds.includes(mate.teammateId)),
                startedTeammateId: teammateId
              })
        if (named !== undefined) {
          // A claim that fails does not hold up a run that has already
          // started: the board is behind, not broken.
          await rooms.updateTask({ roomId, op: 'assign', taskId: named, ownerId: teammateId }).catch(() => undefined)
        }
        // Said HERE, not after the loop: this is the moment the row can
        // appear, and every member after this one is still to be asked.
        sendToWindow({
          kind: 'mission-started',
          runId: attempt.data.runId,
          missionId: attempt.data.missionId,
          teammateId,
          prompt: text,
          data: attempt.data,
          startedBy: { kind: 'room', roomId, postId: post.postId }
        })
      }
      // The post as it now stands, so the answer carries who started, who is
      // waiting and who was refused rather than the empty one written above.
      post = (await rooms.get(roomId))?.posts.find((entry) => entry.postId === post.postId) ?? post
      return { ok: true, data: { post, refused: refusals } } as const
    })

    const routineIO = createRoutineIO({
      // The starter routines, beside the app as the recall model is (0.615, routine-templates.ts).
      templates: app.isPackaged ? join(process.resourcesPath, 'routines') : join(__dirname, '../../resources/routines'),
      routines, team: () => teammates.list(), connectors: () => connectorReader.names(),
      workspace: () => workspaceChosen ? memoryWorkspaceId : undefined,
      run: (id, values) => routineRunner === undefined ? Promise.resolve(routineRejected('That routine could not be started.')) : routineRunner.run(id, undefined, values),
      pickFolder: async () => {
        const scripted = !app.isPackaged ? process.env.LOCUST_ROUTINE_FOLDER_PATH : undefined
        if (scripted) return (await stat(scripted)).isDirectory() ? scripted : undefined
        return (await dialog.showOpenDialog({ title: 'Choose a routine input folder', properties: ['openDirectory'] })).filePaths[0]
      },
      pickImport: async () => {
        const scripted = !app.isPackaged ? process.env.LOCUST_ROUTINE_FILE_PATH : undefined
        return scripted || (await dialog.showOpenDialog({ title: 'Import a routine', filters: [{ name: 'Locust routine', extensions: ['md', 'locust-routine.json'] }], properties: ['openFile'] })).filePaths[0]
      },
      pickExport: async (name) => {
        const scripted = !app.isPackaged ? process.env.LOCUST_ROUTINE_FILE_PATH : undefined
        return scripted || (await dialog.showSaveDialog({ title: 'Export a routine', defaultPath: join(app.getPath('downloads'), name), filters: [name.endsWith('.md') ? { name: 'Locust routine (Markdown)', extensions: ['md'] } : { name: 'Locust routine', extensions: ['locust-routine.json'] }], properties: ['showOverwriteConfirmation'] })).filePath
      }
    })
    routineFolderChosen = routineIO.folderWasChosen
    const routineIORequest = async (event: Electron.IpcMainInvokeEvent, work: () => Promise<unknown>): Promise<unknown> => {
      if (!fromOwnWindow(event)) return routineRejected('That routine request was rejected.')
      try { return await work() } catch (error) { return routineRejected(error instanceof Error ? error.message : 'That routine request could not be completed.') }
    }
    ipcMain.handle(ROUTINE_FOLDER_CHANNEL, (event) => routineIORequest(event, () => routineIO.folder()))
    ipcMain.handle(ROUTINE_EXPORT_CHANNEL, (event, request: unknown) => routineIORequest(event, () => routineIO.export(request)))
    ipcMain.handle(ROUTINE_IMPORT_PREVIEW_CHANNEL, (event) => routineIORequest(event, () => routineIO.preview()))
    ipcMain.handle(ROUTINE_TEMPLATES_CHANNEL, (event) => routineIORequest(event, () => routineIO.templates()))
    ipcMain.handle(ROUTINE_TEMPLATE_PREVIEW_CHANNEL, (event, id: unknown) => routineIORequest(event, () => routineIO.previewTemplate(id)))
    ipcMain.handle(ROUTINE_IMPORT_CHANNEL, (event, request: unknown) => routineIORequest(event, () => routineIO.import(request)))

    ipcMain.handle(ROUTINE_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { ok: false, error: { code: 'ROUTINES_UNAVAILABLE', message: 'Routines are unavailable.' } } as const
      try {
        await routineRunner?.reconcile()
        return { ok: true, data: { routines: await routines.list() } } as const
      } catch {
        return { ok: false, error: { code: 'ROUTINES_UNAVAILABLE', message: 'Routines could not be read.' } } as const
      }
    })
    for (const [channel, replace] of [[ROUTINE_HOOK_READ, false], [ROUTINE_HOOK_NEW, true]] as const) {
      ipcMain.handle(channel, (event, id: unknown) => routineIORequest(event, async () => {
        if (typeof id !== 'string' || (await routines.get(id)) === undefined) return routineRejected('That routine no longer exists.')
        return { ok: true, data: await routineHooks.state(id, replace) }
      }))
    }

    ipcMain.handle(ROUTINE_CREATE_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return routineRejected('The routine could not be saved.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        const routine = await routines.create({
          name: input.name,
          teammateId: input.teammateId,
          route: input.route,
          steps: input.steps,
          learnedFrom: input.learnedFrom,
          ...(input.schedule === undefined ? {} : { schedule: input.schedule }),
          // M15: the folder it is made in, so a schedule runs it only there.
          ...(workspaceChosen ? { workspaceId: memoryWorkspaceId } : {}),
          // Who takes each step (0.435); the store checks it.
          ...(input.handOffs === undefined ? {} : { handOffs: input.handOffs }),
          ...(input.inCopy === true ? { inCopy: true } : {}),
          ...(input.untilCheck === undefined ? {} : { untilCheck: input.untilCheck }),
          ...(input.inputs === undefined ? {} : { inputs: input.inputs })
        })
        await routineHooks.sync().catch(() => note('routine', 'The routine was saved, but its link could not be confirmed. Check Settings > Phone and open the routine again.'))
        return { ok: true, data: { routine } } as const
      } catch (error) {
        return routineRejected(error instanceof Error ? error.message : 'That routine could not be saved.')
      }
    })

    ipcMain.handle(ROUTINE_UPDATE_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return routineRejected('The routine could not be changed.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        const routine = await routines.update({
          routineId: input.routineId,
          name: input.name,
          steps: input.steps,
          ...(input.schedule === undefined ? {} : { schedule: input.schedule }),
          ...(input.handOffs === undefined ? {} : { handOffs: input.handOffs }),
          ...(input.mode === undefined ? {} : { mode: input.mode }),
          ...(typeof input.inCopy === 'boolean' ? { inCopy: input.inCopy } : {}),
          ...(input.untilCheck === undefined ? {} : { untilCheck: input.untilCheck }),
          ...(input.inputs === undefined ? {} : { inputs: input.inputs })
        })
        await routineHooks.sync().catch(() => note('routine', 'The routine was changed, but its link could not be confirmed. Check Settings > Phone and open the routine again.'))
        return { ok: true, data: { routine } } as const
      } catch (error) {
        return routineRejected(error instanceof Error ? error.message : 'That routine could not be changed.')
      }
    })

    ipcMain.handle(ROUTINE_REMOVE_CHANNEL, async (event, routineId: unknown) => {
      if (!fromOwnWindow(event)) return routineRejected('The routine could not be removed.')
      try {
        await routines.remove(routineId)
        await routineHooks.sync().catch(() => note('routine', 'The routine was removed. Its link will be closed when the relay reconnects.'))
        // Its copy, if it worked in one, goes with it (0.533): nothing of it reached the folder.
        if (typeof routineId === 'string') await routineCopies.discard(routineId).catch(() => undefined)
        return { ok: true, data: {} } as const
      } catch {
        return routineRejected('That routine could not be removed.')
      }
    })

    // Pause or resume a routine's schedule (0.705). Run still runs a paused one.
    ipcMain.handle(ROUTINE_PAUSE_CHANNEL, async (event, routineId: unknown, paused: unknown) => {
      if (!fromOwnWindow(event)) return routineRejected('The routine could not be changed.')
      try {
        const routine = await routines.setPaused(routineId, paused, new Date().toISOString())
        if (routine === undefined) return routineRejected('That routine no longer exists.')
        // The tray's "next routine" line counts paused routines out.
        sendToWindow({ kind: 'routine-recovery-changed' })
        return { ok: true, data: { routine } } as const
      } catch {
        return routineRejected('That routine could not be changed.')
      }
    })

    ipcMain.handle(ROUTINE_RUN_CHANNEL, async (event, routineId: unknown, values: unknown) => {
      if (!fromOwnWindow(event)) return routineRejected('The routine could not be started.')
      if (typeof routineId !== 'string' || routineRunner === undefined) return routineRejected('That routine could not be started.')
      if (!workspaceChosen) return routineRejected(NO_WORKSPACE_MESSAGE)
      return routineIORequest(event, () => routineIO.run(routineId, values))
    })

    /*
     * A COPY ROUTINE'S CHANGES, SETTLED BY THE PERSON (0.533). Keep writes them
     * into the folder the copy was made from -- and nothing at all when the
     * person has since changed any of those same files; Discard throws the copy
     * away; Open shows it in the file manager to look at first.
     */
    ipcMain.handle(ROUTINE_SETTLE_CHANNEL, async (event, request: unknown) => {
      const refuse = (message: string) => ({ ok: false, message }) as const
      if (!fromOwnWindow(event)) return refuse('That could not be done.')
      const { routineId, decision } = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      if (typeof routineId !== 'string' || (decision !== 'keep' && decision !== 'discard' && decision !== 'open')) return refuse('That could not be done.')
      const routine = await routines.get(routineId).catch(() => undefined)
      const staged = routine?.staged
      if (routine === undefined || staged === undefined) return refuse('Those changes were already kept or discarded.')
      if (decision === 'open') {
        const failed = await shell.openPath(routineCopies.path(routineId))
        return failed.length === 0 ? ({ ok: true, message: '' } as const) : refuse(`The copy could not be opened: ${failed}`)
      }
      if (decision === 'discard') {
        await routines.settleStaged(routineId, staged.attemptId)
        await routineCopies.discard(routineId).catch(() => undefined)
        sendToWindow({ kind: 'routine-recovery-changed' })
        return { ok: true, message: `Discarded. Nothing from that run of "${routine.name}" reached the folder.` } as const
      }
      const kept = await routineCopies.keep(routineId).catch((error: unknown) => (error instanceof Error ? error : new Error('The changes could not be kept.')))
      if (kept instanceof Error) return refuse(kept.message)
      if (kept.kind === 'your-changes') {
        const named = kept.files.length <= 3 ? kept.files.join(', ') : `${kept.files.slice(0, 3).join(', ')} and ${String(kept.files.length - 3)} more`
        return refuse(`Not kept: you have changed ${named} since it ran, so nothing was written. Discard its changes, or put those files back and Keep again.`)
      }
      await routines.settleStaged(routineId, staged.attemptId)
      await routineCopies.discard(routineId).catch(() => undefined)
      sendToWindow({ kind: 'routine-recovery-changed' })
      return {
        ok: true,
        message: kept.kind === 'nothing'
          ? 'Nothing to write: the folder already matches.'
          : `Kept: ${String(kept.files.length)} file${kept.files.length === 1 ? '' : 's'} written into the folder.`
      } as const
    })

    ipcMain.handle(ROUTINE_RECOVERY_CHANNEL, (event, request: unknown) =>
      decideRoutineRecovery(request, fromOwnWindow(event), workspaceChosen, routineRunner))

    // Memory. Every answer carries the whole list so the screen never
    // shows a state the file does not hold.
    const memoryRejected = (message: string) => ({ ok: false, error: { code: 'MEMORY_REJECTED', message } }) as const
    const memoryList = async () => {
      // One read for both lists, so a memory is never shown in both or neither.
      const held = await memories.snapshot()
      // A1.3: for the memories this folder reads, the named files that
      // changed after they were written -- shown on the Memory screen.
      const outOfDate: Record<string, readonly string[]> = {}
      await Promise.all(
        held.memories
          .filter((memory) => memory.status === 'kept' && (memory.scope === 'global' || memory.workspaceId === memoryWorkspaceId))
          .map(async (memory) => {
            const files = await changedSince(workspacePath, memory.text, lastWritten(memory)).catch(() => [])
            if (files.length > 0) outOfDate[memory.memoryId] = files
          })
      )
      return {
        ok: true,
        data: {
          memories: held.memories,
          forgotten: held.forgotten,
          changedSince: outOfDate,
          ...(held.briefTrackingSince === undefined ? {} : { briefTrackingSince: held.briefTrackingSince }),
          workspaceId: memoryWorkspaceId,
          workspaceName: memoryWorkspaceName
        }
      } as const
    }
    ipcMain.handle(MEMORY_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return memoryRejected('Memory could not be read.')
      try {
        return await memoryList()
      } catch {
        return memoryRejected('Memory could not be read.')
      }
    })
    ipcMain.handle(MEMORY_ADD_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return memoryRejected('The memory could not be kept.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        await memories.add({
          text: input.text,
          scope: input.scope,
          workspaceId: memoryWorkspaceId,
          workspaceName: memoryWorkspaceName,
          by: { name: 'you' },
          status: 'kept'
        })
        return await memoryList()
      } catch (error) {
        return memoryRejected(error instanceof Error ? error.message : 'The memory could not be kept.')
      }
    })
    ipcMain.handle(MEMORY_UPDATE_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return memoryRejected('The memory could not be changed.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        await memories.update({ memoryId: input.memoryId, text: input.text, enabled: input.enabled, keep: input.keep })
        return await memoryList()
      } catch (error) {
        return memoryRejected(error instanceof Error ? error.message : 'The memory could not be changed.')
      }
    })
    ipcMain.handle(MEMORY_REMOVE_CHANNEL, async (event, memoryId: unknown) => {
      if (!fromOwnWindow(event)) return memoryRejected('The memory could not be removed.')
      try {
        await memories.remove(memoryId, { name: 'you' })
        return await memoryList()
      } catch {
        return memoryRejected('The memory could not be removed.')
      }
    })
    // Skills from a GitHub repository (0.710, skill-library.ts). Look, then keep what was looked at.
    const skillLibraryRejected = (error: unknown, fallback: string) =>
      ({ ok: false, error: { code: 'SKILL_LIBRARY_REJECTED', message: error instanceof SkillLibraryError ? error.message : fallback } }) as const
    ipcMain.handle(SKILL_LIBRARY_PREVIEW_CHANNEL, async (event, link: unknown) => {
      if (!fromOwnWindow(event)) return skillLibraryRejected(undefined, 'Locust could not look at that repository.')
      try {
        return { ok: true, data: await skillLibrary.preview(String(link ?? '')) } as const
      } catch (error) {
        note('skill-library', `preview failed: ${error instanceof Error ? error.message : String(error)}`)
        return skillLibraryRejected(error, 'Locust could not look at that repository.')
      }
    })
    ipcMain.handle(SKILL_LIBRARY_INSTALL_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return skillLibraryRejected(undefined, 'Nothing was kept.')
      try {
        const sources = await skillLibrary.install((typeof request === 'object' && request !== null ? request : {}) as SkillLibraryInstallRequest)
        return { ok: true, data: { sources } } as const
      } catch (error) {
        note('skill-library', `keep failed: ${error instanceof Error ? error.message : String(error)}`)
        return skillLibraryRejected(error, 'Nothing was kept. Try again.')
      }
    })
    ipcMain.handle(SKILL_LIBRARY_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return skillLibraryRejected(undefined, 'Locust could not read the kept skills.')
      try {
        return { ok: true, data: { sources: await skillLibrary.list() } } as const
      } catch (error) {
        return skillLibraryRejected(error, 'Locust could not read the kept skills.')
      }
    })
    ipcMain.handle(SKILL_LIBRARY_REMOVE_CHANNEL, async (event, source: unknown) => {
      if (!fromOwnWindow(event)) return skillLibraryRejected(undefined, 'Nothing was removed.')
      try {
        return { ok: true, data: { sources: await skillLibrary.remove(String(source ?? '')) } } as const
      } catch (error) {
        return skillLibraryRejected(error, 'Nothing was removed. Try again.')
      }
    })
    ipcMain.handle(MEMORY_RESTORE_CHANNEL, async (event, memoryId: unknown) => {
      if (!fromOwnWindow(event)) return memoryRejected('The memory could not be put back.')
      try {
        await memories.restore(memoryId)
        return await memoryList()
      } catch (error) {
        return memoryRejected(error instanceof Error ? error.message : 'The memory could not be put back.')
      }
    })
    ipcMain.handle(MEMORY_CLEAR_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) return memoryRejected('Memory could not be cleared.')
      const input = (typeof request === 'object' && request !== null ? request : {}) as Record<string, unknown>
      try {
        await memories.clear(input.scope === 'all' ? {} : { workspaceId: memoryWorkspaceId })
        return await memoryList()
      } catch {
        return memoryRejected('Memory could not be cleared.')
      }
    })

    claudeContextReader = createClaudeContextReader({
      file: join(app.getPath('userData'), 'claude-context-launches.json'),
      resolve: async (id) => {
        const newest = await newestTurnOf(missionLedger, id)
        const mission = await missionLedger.getMission(newest)
        if (mission?.metadata.runtime !== 'claude' || (await cloudTurns.of(newest)) !== undefined) return undefined
        const sessionId = runtimeThreadIdOf(mission)
        if (sessionId === undefined) return undefined
        const ancestors = [newest]
        let prior = mission.metadata.continuesFrom?.missionId
        while (prior !== undefined && !ancestors.includes(prior) && ancestors.length < 512) {
          ancestors.push(prior)
          prior = (await missionLedger.getMission(prior))?.metadata.continuesFrom?.missionId
        }
        const cwd = await references.cwd(newest) ?? await terminalFolder({ missionId: newest }, {
          current: () => workspacePath,
          mission: async (missionId) => (await missionLedger.getMission(missionId))?.metadata,
          folderOf: workableFolderOf,
          ownerOf: async (missionId) => (await teammates.missionOwners())[missionId],
          teammates: () => teammates.list()
        })
        return { missionId: newest, sessionId, busy: codexMissions.contextBusy(ancestors),
          ...(accountFromRoute(mission.metadata.resolvedRouteId) === undefined ? {} : { account: accountFromRoute(mission.metadata.resolvedRouteId) }),
          fallback: recordedClaudeContextLaunch(mission.metadata.command, cwd, mission.metadata.sandbox)
        }
      },
      prepare: async (launch, turn) => {
        const found = (await discoverForAccount('claude', turn.account)).find((entry) => entry.id === 'claude')?.executable
        if (found === undefined) throw new Error('Claude is unavailable.')
        return prepareClaudeContextLaunch(launch, turn, {
          ...(found.env === undefined ? {} : { env: found.env }),
          permission: async (runId, missionId, cwd) => (await permissionHost.register({ runId, missionId, cwd })).configPath,
          browser: async (runId, missionId, cwd) => (await browserHost.registerRun({ runId, missionId, teammateLabel: 'Context inspection', cwd, askApproval: async () => ({ approved: false }) })).configPath,
          skills: async (profile) => prepareClaudeSkills({ scratchRoot: claudeSkillCopies, workspacePath: launch.cwd, ownSkills: profile.ownSkills, libraryFolder: skillLibrary.skillsFolder, auto: profile.auto }),
          release: async (runId) => { await Promise.all([permissionHost.release(runId), browserHost.releaseRun(runId)]) }
        })
      },
      ...(!app.isPackaged && process.env.LOCUST_CONTEXT_FIXTURE_PATH ? { run: async () => {
        const { readFile } = await import('node:fs/promises')
        return readFile(process.env.LOCUST_CONTEXT_FIXTURE_PATH!, 'utf8')
      } } : {})
    })
    ipcMain.handle(CONTEXT_BREAKDOWN_CHANNEL, async (event, missionId: unknown) => {
      if (!fromOwnWindow(event) || typeof missionId !== 'string' || !/^[A-Za-z0-9_-]{1,128}$/.test(missionId)) return undefined
      return claudeContextReader?.read(missionId)
    })

    // H3: one mission whole, for a conversation history listed as a row.
    ipcMain.handle(MISSION_READ_CHANNEL, async (event, missionId: unknown) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'MISSION_UNAVAILABLE', message: 'The request was rejected.' } } as const
      }
      return readOneMission(missionLedger, workroom, missionId)
    })

    ipcMain.handle(MISSION_HISTORY_CHANNEL, async (event, known: unknown) => {
      const owner = BrowserWindow.fromWebContents(event.sender)
      if (!owner || !event.senderFrame || event.senderFrame.parent !== null) {
        return {
          ok: false,
          error: {
            code: 'HISTORY_UNAVAILABLE',
            message: 'Local mission history could not be read.'
          }
        } as const
      }
      // The window's own folder decides which conversation it opens on; what
      // it already holds decides which records come without their events.
      const history = await readMissionHistory(missionLedger, workroom, workspacePath, knownDigests(known), join(app.getPath('userData'), 'usage-readings.json'))
      // Every folder listed needs a path to open in (0.458): recovered once, for those from before folders were kept.
      if (history.ok) await recoverFolders(history.data.missions).catch(() => undefined)
      /*
       * DID A DELETED MISSION COME BACK?
       *
       * Colin, 2026-09-15: "im having this issue where when i delete a
       * mission i feel like it keeps coming back, im not sure whats
       * happening or if i can prove it" -- and later, the detail that
       * matters: "usually it reappeared after a little bit."
       *
       * I could not reproduce it. Driven through the real menu, a
       * three-turn conversation lost all three records and the row went. So
       * rather than guess, make the next occurrence evidence itself: the
       * host remembers the ids it deleted this session and says so if one is
       * ever read back out of the ledger.
       *
       * The answer is useful in BOTH directions, which is why it is worth
       * the six lines. A line here means a record genuinely returned and
       * something is rewriting it. NO line, while a row that looks like the
       * deleted one is on screen, means it is a different mission that
       * merely reads the same -- which is easy to believe on a list where
       * every relayed turn is titled "Jimothy asked: ..." and the title is
       * the first line of a prompt.
       *
       * Ids only. They are uuids, not content, and this file is written to
       * be handed to someone.
       */
      if (history.ok && deletedThisSession.size > 0) {
        const back = history.data.missions
          .map((mission) => mission.missionId)
          .filter((missionId) => deletedThisSession.has(missionId))
        if (back.length > 0) {
          for (const missionId of back) deletedThisSession.delete(missionId)
          note('deleted-mission-returned', back.join(' '))
        }
      }
      return withLiveMissionIds(history, [...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()])
    })

    /*
     * What changed, answered once per launch.
     *
     * The FIRST-RUN decision is made here and written down immediately, so a
     * window that asks twice gets the same answer and a window that never
     * asks does not leave the app claiming to be new forever. The file is
     * one line in the profile; losing it costs one extra "what changed",
     * which is the harmless direction.
     */
    const seenVersionFile = join(app.getPath('userData'), 'seen-version.json')
    let changelogAnswer: Promise<AppChangelog> | undefined
    const readChangelogOnce = (): Promise<AppChangelog> => {
      changelogAnswer ??= (async () => {
        const version = app.getVersion()
        let seen: string | undefined
        try {
          seen = JSON.parse(readFileSync(seenVersionFile, 'utf8')).version
        } catch {
          // Never launched, or the file is unreadable: either way this
          // version has not been shown.
        }
        /*
         * NOT written here any more. Reading is not showing: the renderer
         * reads the changelog at mount and holds the banner until a runtime
         * connects, so a launch with nothing connected marked the version
         * seen and the person the hold exists for -- whose runtime was
         * signed out or hanging, who fixed it and came back -- never saw
         * what changed (Fable, pass 1, finding 7). The renderer says when
         * the banner is on screen, on `APP_CHANGELOG_SEEN_CHANNEL`.
         */
        const text = await readChangelog(changelogPaths(process.resourcesPath, app.getAppPath()))
        const all = text === undefined ? [] : changelogEntries(text)
        const entry = all.find((candidate) => candidate.version === version)
        // What's new shows every build; the splash, the big ones this person
        // has not been shown (Colin: "if we have really good big updates
        // where the user has to know things, we can have a splash page").
        const shown = (candidate: ChangelogEntry): AppChangelogEntry => ({
          version: candidate.version,
          ...(candidate.date === undefined ? {} : { date: candidate.date }),
          groups: candidate.groups
        })
        return {
          version,
          ...(entry?.body === undefined ? {} : { body: entry.body }),
          ...(entry?.date === undefined ? {} : { date: entry.date }),
          firstRun: seen !== version,
          entries: all.map(shown),
          splash: splashEntries(all, seen, version).map(shown)
        }
      })()
      return changelogAnswer
    }

    ipcMain.handle(APP_CHANGELOG_SEEN_CHANNEL, (event) => {
      if (!fromOwnWindow(event)) return
      try {
        mkdirSync(app.getPath('userData'), { recursive: true })
        writeFileSync(seenVersionFile, JSON.stringify({ version: app.getVersion() }), 'utf8')
      } catch {
        // Remembering is a convenience; failing to costs one extra banner.
      }
    })

    ipcMain.handle(APP_CHANGELOG_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return { version: app.getVersion(), firstRun: false } as const
      try {
        return await readChangelogOnce()
      } catch {
        return { version: app.getVersion(), firstRun: false } as const
      }
    })

    ipcMain.handle(APP_INFO_CHANNEL, (event) => {
      if (!fromOwnWindow(event)) return { name: 'Locust', version: 'unknown', packaged: app.isPackaged, platform: process.platform, workspaceName: '', workspacePath: '', workspaceMade: false, ledgerPath: '' } as const
      // The version electron-builder stamped, which is the one on the installer.
      return {
        name: 'Locust',
        version: appVersion,
        packaged: app.isPackaged,
        platform: process.platform,
        organizationPolicy,
        workspaceName: workspaceChosen ? basename(workspacePath) || workspacePath : '',
        workspacePath: workspaceChosen ? workspacePath : '',
        workspaceMade,
        // A folder like home or .claude (0.790): the folder chip says so, in amber, whatever was answered.
        ...(workspaceChosen && folderCaution(workspacePath) !== undefined ? { workspaceCaution: folderCaution(workspacePath)! } : {}),
        // Where the receipts live. The renderer needs it for one thing: when a
        // write to the ledger fails, the card offers to open the folder, and
        // an offer to show someone a place has to know which place.
        ledgerPath: ledgerDirectory
      } as const
    })

    // Updates. A packaged build can replace itself; a development build
    // cannot, and says so rather than reporting itself up to date.
    // Which builds it takes: the tester lane unless the person chose every
    // build (update-lane.ts). Its own small file, like the window's.
    const laneFile = join(app.getPath('userData'), 'update-lane.json')
    const lane = (() => {
      try {
        return updateLaneFrom(JSON.parse(readFileSync(laneFile, 'utf8')))
      } catch {
        return TESTER_LANE
      }
    })()
    const installedCopy = existsSync(join(dirname(process.execPath), 'Uninstall Locust.exe')) || linuxInstall !== undefined
    // locust:// links open the installed copy (0.770, locust-links.ts `claimLinks`); a test's copy never claims them.
    try {
      claimLinks({ platform: process.platform, packaged: app.isPackaged, installed: installedCopy, argv: process.argv, host: app })
    } catch {
      // A registry that refuses leaves links unclaimed, and the app as it was.
    }
    /*
     * A MAC UPDATES ITSELF (0.516, mac-self-update.ts): a Locust.app in a folder
     * it can write, replaced by Locust's own download. macOS's updater would
     * refuse the unsigned build. `LOCUST_MAC_UPDATE_TEST_DMG` and `_VERSION`
     * are the Mac CI's check of the swap (mac-update-smoke.mjs), never set by
     * the app.
     */
    // A driven copy never replaces the installed app, on any system (mayUpdateItself, the first Mac run).
    const updatesItself = mayUpdateItself(process.argv, process.env)
    const macBundle = process.platform === 'darwin' && app.isPackaged && updatesItself ? macSelfUpdateTarget(process.execPath) : undefined
    const macTestImage = process.env.LOCUST_MAC_UPDATE_TEST_DMG !== undefined && process.env.LOCUST_MAC_UPDATE_TEST_VERSION !== undefined
      ? { path: process.env.LOCUST_MAC_UPDATE_TEST_DMG, version: process.env.LOCUST_MAC_UPDATE_TEST_VERSION }
      : undefined
    // The Linux CI's check of an AppImage update (linux-update-smoke.mjs): a local generic feed, never set by the app.
    if (linuxUpdateTest?.feedUrl !== undefined) autoUpdater.setFeedURL(linuxUpdateTest.feedUrl)
    const updates = createUpdateService({
      updater: macBundle === undefined
        ? autoUpdater
        : createMacUpdater({
            currentVersion: appVersion,
            arch: process.arch,
            bundle: macBundle,
            cacheFolder: join(app.getPath('userData'), 'mac-updates'),
            quit: () => app.quit(),
            ...(macTestImage === undefined ? {} : { testImage: macTestImage })
          }),
      everyBuild: lane.everyBuild,
      currentVersion: appVersion,
      // A .deb or .rpm is the package manager's to replace: the new package is handed over instead (0.789).
      ...(linuxInstall === 'deb' || linuxInstall === 'rpm'
        ? { packageUrl: (files: readonly { readonly url?: unknown }[] | undefined) => linuxPackageUrl(linuxInstall, files, DOWNLOADS) ?? SITE }
        : {}),
      // Not on macOS until the build is signed: its updater refuses an unsigned app.
      /*
       * AND ONLY THE INSTALLED COPY (0.507). Every copy of Locust shares one
       * update cache (%LOCALAPPDATA%\@teammatedesktop-updater), and the
       * installer it holds writes into the INSTALLED folder and closes the
       * Locust running there. A test copy -- a beta tester's in %TEMP%, a
       * drive's win-unpacked -- that quit with an update pending ran that
       * installer: on 2026-09-30 it closed Colin's own Locust twice, at 15:48
       * and 15:56, each time in the middle of the run that was testing it.
       * The installed copy has its uninstaller beside it; a copy does not.
       * `--update-check-only` lets the update smoke's copy ask the real feed,
       * with nothing downloaded or installed.
       */
      supported: app.isPackaged && (macBundle !== undefined || (process.platform !== 'darwin' && (installedCopy || process.argv.includes('--update-check-only')))),
      // Why not, when it is not: said in Settings in place of a guess (the first Mac run).
      heldBack: !app.isPackaged ? 'development' : !updatesItself ? 'driven' : 'copy',
      checkOnly: macBundle === undefined && (!installedCopy || !updatesItself),
      liveMissionCount: () =>
        codexMissions.liveMissionIds().length + antigravityMissions.liveMissionIds().length,
      requestQuit: (finalise) => {
        // Held for the shutdown handler: an installer can only be launched
        // once the ledger is safely on disk.
        quitFinaliser = finalise
        app.quit()
      },
      onStateChange: (state) => {
        for (const target of BrowserWindow.getAllWindows()) {
          if (!target.isDestroyed()) target.webContents.send(APP_UPDATE_STATE_CHANNEL, state)
        }
      }
    })
    // One check a few seconds after launch, so a person is told a new version
    // exists without ever being asked to go looking -- and again every hour
    // while the app stays open, a downloaded version waiting or not, never
    // while a teammate is working (C2, 0.367: it looked only at launch, known
    // issue 5; 0.820: hourly, and on past a download).
    setTimeout(() => {
      void updates.check()
    }, 8_000)
    updates.startPeriodicChecks()

    ipcMain.handle(APP_UPDATE_CHECK_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The request was rejected.' } } as const
      }
      return updates.check()
    })

    const loginItems = createLoginItem({
      host: {
        setLoginItemSettings: (settings) => { app.setLoginItemSettings(settings) },
        getLoginItemSettings: (query) => app.getLoginItemSettings(query)
      },
      // The smoke sets the seam so a development copy can prove the Run key,
      // then puts the key back. A normal development launch does not.
      packaged: app.isPackaged || process.env.LOCUST_LOGIN_ITEM_SMOKE === '1',
      execPath: process.execPath,
      platform: process.platform
    })
    const backgroundFile = (): string => join(app.getPath('userData'), BACKGROUND_FILE)
    ipcMain.handle(LOGIN_ITEM_GET_CHANNEL, (event) => {
      if (!fromOwnWindow(event)) return { openAtLogin: false, available: false }
      return loginItems.read()
    })
    ipcMain.handle(LOGIN_ITEM_SET_CHANNEL, (event, openAtLogin: unknown) => {
      if (!fromOwnWindow(event) || typeof openAtLogin !== 'boolean') return loginItems.read()
      return loginItems.set(openAtLogin)
    })
    ipcMain.handle(APPEARANCE_GET_CHANNEL, () => appearanceNow())
    ipcMain.handle(APPEARANCE_SET_CHANNEL, (event, theme: unknown) => {
      if (!fromOwnWindow(event) || !THEME_CHOICES.includes(theme as ThemeChoice)) return appearanceNow()
      themeChoice = theme as ThemeChoice
      try {
        writeTheme(appearanceFile(), themeChoice)
      } catch {
        // Kept for this run; the next start reads Dark again.
      }
      nativeTheme.themeSource = themeChoice
      broadcastAppearance()
      return appearanceNow()
    })
    ipcMain.handle(KEEP_RUNNING_GET_CHANNEL, (event) => {
      if (!fromOwnWindow(event)) return { keepRunning: false }
      return { keepRunning: readKeepRunning(backgroundFile()) }
    })
    ipcMain.handle(KEEP_RUNNING_SET_CHANNEL, (event, keep: unknown) => {
      if (!fromOwnWindow(event) || typeof keep !== 'boolean') return { keepRunning: readKeepRunning(backgroundFile()) }
      writeKeepRunning(backgroundFile(), keep)
      return { keepRunning: keep }
    })

    ipcMain.handle(APP_UPDATE_LANE_CHANNEL, async (event, everyBuild: unknown) => {
      if (!fromOwnWindow(event) || typeof everyBuild !== 'boolean') {
        return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The request was rejected.' } } as const
      }
      await writeFile(laneFile, JSON.stringify({ everyBuild }), 'utf8').catch(() => undefined)
      updates.setEveryBuild(everyBuild)
      // Looked at again on the new lane at once, so the switch says what it did.
      return updates.check()
    })

    ipcMain.handle(APP_UPDATE_INSTALL_CHANNEL, (event) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The request was rejected.' } } as const
      }
      return updates.install()
    })

    // The teammates' own worktrees: listed from git, removed on request when
    // no run is live in them. The branch stays either way.
    const worktreesRejected = (message: string) => ({ ok: false, error: { code: 'WORKTREES_UNAVAILABLE', message } }) as const
    // The window's folder's, read live (0.458).
    const currentWorktrees = () => (workspaceChosen ? createWorktreeManager({ workspacePath, note: detail => note('worktree-add', detail) }) : undefined)
    const worktreeList = async () => {
      const worktrees = currentWorktrees()
      if (worktrees === undefined) return { ok: true, data: { worktrees: [], reason: 'No project folder is chosen.' } } as const
      const probe = await worktrees.probe()
      const [roster, owners] = await Promise.all([teammates.list(), teammates.missionOwners()])
      const live = new Set(
        [...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()]
          .map((missionId) => owners[missionId])
          .filter((owner): owner is string => owner !== undefined)
      )
      const listed = (await worktrees.list()).map((tree) => ({
        teammateId: tree.teammateId,
        teammateName: roster.find((entry) => entry.teammateId === tree.teammateId)?.name,
        branch: tree.branch,
        path: tree.path,
        busy: live.has(tree.teammateId)
      }))
      return { ok: true, data: { worktrees: listed, reason: probe.reason } } as const
    }
    ipcMain.handle(WORKTREE_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return worktreesRejected('The request was rejected.')
      try {
        return await worktreeList()
      } catch {
        return worktreesRejected('The worktrees could not be listed.')
      }
    })
    ipcMain.handle(WORKTREE_REMOVE_CHANNEL, async (event, teammateId: unknown, discard: unknown) => {
      if (!fromOwnWindow(event)) return worktreesRejected('The request was rejected.')
      const worktrees = currentWorktrees()
      if (typeof teammateId !== 'string' || worktrees === undefined) return worktreesRejected('That worktree could not be removed.')
      try {
        const current = await worktreeList()
        if (current.ok && current.data.worktrees.some((tree) => tree.teammateId === teammateId && tree.busy)) {
          return worktreesRejected('A run is live in that worktree. Stop it first.')
        }
        const agreed = Array.isArray(discard) && discard.every((entry) => typeof entry === 'string') ? (discard as string[]) : undefined
        await worktrees.remove(teammateId, agreed === undefined ? {} : { discard: agreed })
        return await worktreeList()
      } catch (error) {
        if (error instanceof WorktreeHasChangesError) {
          return { ok: false, error: { code: 'WORKTREE_HAS_CHANGES', message: error.message, changes: error.changes } } as const
        }
        return worktreesRejected(error instanceof Error ? error.message : 'That worktree could not be removed.')
      }
    })

    /*
     * REVIEW CHANGES (0.439): a teammate's own branch against where it left
     * the person's branch, whole or turn by turn. Reads only. The repository
     * is the one its tree was cut from: its own folder's, else the project's.
     */
    const reviewRejected = (message: string) => ({ ok: false, error: { code: 'REVIEW_UNAVAILABLE', message } }) as const
    const branchManagerFor = async (teammateId: unknown) => {
      if (typeof teammateId !== 'string') return undefined
      const teammate = (await teammates.list()).find((entry) => entry.teammateId === teammateId)
      if (teammate === undefined) return undefined
      if (teammate.folder !== undefined) return createWorktreeManager({ workspacePath: teammate.folder, note: detail => note('worktree-add', detail) })
      return currentWorktrees()
    }
    ipcMain.handle(WORKTREE_REVIEW_CHANNEL, async (event, teammateId: unknown) => {
      if (!fromOwnWindow(event)) return reviewRejected('The request was rejected.')
      const manager = await branchManagerFor(teammateId).catch(() => undefined)
      if (manager === undefined) return reviewRejected('That teammate has no own branch here.')
      try {
        return { ok: true, data: await manager.review(teammateId as string) } as const
      } catch (error) {
        return reviewRejected(error instanceof Error ? error.message : 'The branch could not be read. Nothing on it was changed.')
      }
    })
    /*
     * LAND IT (0.440): the teammate's branch onto the person's, as one commit
     * of theirs (worktrees.ts `land`). Never while a run of the teammate's is
     * live: its checkpoint would race the landing.
     */
    const teammateBusy = async (teammateId: string): Promise<boolean> => {
      const owners = await teammates.missionOwners()
      return [...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()].some((missionId) => owners[missionId] === teammateId)
    }
    const landTarget = async (teammateId: unknown) => {
      if (typeof teammateId !== 'string') return undefined
      const teammate = (await teammates.list()).find((entry) => entry.teammateId === teammateId)
      const manager = await branchManagerFor(teammateId)
      return teammate === undefined || manager === undefined ? undefined : { teammate: { teammateId: teammate.teammateId, name: teammate.name }, manager }
    }
    ipcMain.handle(WORKTREE_LAND_PREVIEW_CHANNEL, async (event, teammateId: unknown) => {
      if (!fromOwnWindow(event)) return reviewRejected('The request was rejected.')
      const target = await landTarget(teammateId).catch(() => undefined)
      if (target === undefined) return reviewRejected('That teammate has no own branch here.')
      try {
        const preview = await target.manager.landPreview(target.teammate)
        const busy = await teammateBusy(target.teammate.teammateId)
        return { ok: true, data: busy && preview.block?.kind !== 'nothing' ? { ...preview, block: { kind: 'busy' } } : preview } as const
      } catch (error) {
        return reviewRejected(error instanceof Error ? error.message : 'The branch could not be read. Nothing on it was changed.')
      }
    })
    ipcMain.handle(WORKTREE_LAND_CHANNEL, async (event, teammateId: unknown, message: unknown) => {
      if (!fromOwnWindow(event)) return reviewRejected('The request was rejected.')
      const target = await landTarget(teammateId).catch(() => undefined)
      if (target === undefined) return reviewRejected('That teammate has no own branch here.')
      if (await teammateBusy(target.teammate.teammateId)) return { ok: true, data: { kind: 'blocked', block: { kind: 'busy' } } } as const
      try {
        return { ok: true, data: await target.manager.land(target.teammate, typeof message === 'string' ? message.slice(0, 20_000) : '') } as const
      } catch (error) {
        return reviewRejected(`${error instanceof Error ? error.message : 'The landing could not be made.'} Your checkout is as it was.`)
      }
    })
    ipcMain.handle(WORKTREE_RESOLVE_CHANNEL, async (event, teammateId: unknown) => {
      if (!fromOwnWindow(event)) return reviewRejected('The request was rejected.')
      const target = await landTarget(teammateId).catch(() => undefined)
      if (target === undefined) return reviewRejected('That teammate has no own branch here.')
      if (await teammateBusy(target.teammate.teammateId)) return reviewRejected(`${target.teammate.name} is working. Ask again when the turn ends.`)
      try {
        const preview = await target.manager.landPreview(target.teammate)
        if (preview.onto === undefined) return reviewRejected('Your checkout is not on a branch, so there is nothing to merge from.')
        // A merge already begun is not begun again: its files are still the ones to resolve.
        const files = preview.block?.kind === 'merging' || preview.block?.kind === 'markers'
          ? (preview.block.kind === 'markers' ? preview.block.files : preview.files)
          : await target.manager.startResolving(target.teammate.teammateId)
        return { ok: true, data: { onto: preview.onto, files } } as const
      } catch (error) {
        return reviewRejected(`${error instanceof Error ? error.message : 'The merge could not be begun.'} Your checkout was not touched.`)
      }
    })
    ipcMain.handle(WORKTREE_TURN_DIFF_CHANNEL, async (event, teammateId: unknown, sha: unknown) => {
      if (!fromOwnWindow(event)) return reviewRejected('The request was rejected.')
      const manager = await branchManagerFor(teammateId).catch(() => undefined)
      if (manager === undefined || typeof sha !== 'string') return reviewRejected('That teammate has no own branch here.')
      try {
        return { ok: true, data: { diff: await manager.turnDiff(teammateId as string, sha) } } as const
      } catch (error) {
        return reviewRejected(error instanceof Error ? error.message : 'That turn could not be read. The branch is as it was.')
      }
    })

    // What each runtime has set up for itself -- MCP servers and hooks --
    // read from its own files so nothing fires unseen (Colin, 2026-09-05).
    ipcMain.handle(RUNTIME_SETUP_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'SETUP_UNAVAILABLE', message: 'The request was rejected.' } } as const
      }
      try {
        const brief = workspaceChosen ? await readWorkspaceBrief(workspacePath).catch(() => undefined) : undefined
        return {
          ok: true,
          data: {
            runtimes: await readRuntimeSetup({ workspacePath: workspaceChosen ? workspacePath : undefined }),
            workspaceBrief: brief === undefined ? null : { lines: brief.lines, truncated: brief.truncated }
          }
        } as const
      } catch {
        return { ok: false, error: { code: 'SETUP_UNAVAILABLE', message: 'The runtimes\' own configuration could not be read.' } } as const
      }
    })

    ipcMain.handle(MISSION_STORAGE_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) {
        return {
          ok: false,
          error: { code: 'STORAGE_UNAVAILABLE', message: 'The request was rejected.' }
        } as const
      }
      return readStorageReport(missionLedger)
    })

    ipcMain.handle(MISSION_PRUNE_CHANNEL, async (event, request: unknown) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The request was rejected.' } } as const
      }
      const response = await pruneMissionRecords(
        missionLedger,
        request,
        () => [...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()],
        () => new Date()
      )
      // Ownership is NOT dropped here: pruned missions go to the trash, and a
      // restore must bring each back to its teammate -- it came back owned by
      // nobody (L8). Emptying the trash drops the owners, as a single delete's.
      return response
    })

    /*
     * BACK UP AND RESTORE (0.614, profile-backup.ts). The folder is the
     * person's pick; the backup is a new folder inside it; a restore is read
     * by the same reader as its preview, refused while a run is going, and
     * applied at the next start, so Locust restarts once it is written down.
     */
    const profileFolderOf = (folder: unknown): string | undefined => (typeof folder === 'string' && isAbsolute(folder) ? folder : undefined)
    ipcMain.handle(PROFILE_PICK_FOLDER_CHANNEL, async (event, purpose: unknown) => {
      if (!fromOwnWindow(event)) return undefined
      /*
       * `LOCUST_TEST_PICK_FOLDER` is a test seam, as LOCUST_INSTALL_DIR is: a
       * drive cannot answer a native folder dialog, so it names a FILE whose
       * text is the folder picked, read at the moment of the pick (an empty or
       * missing file is a cancel). drive-backup-and-restore.mjs uses it.
       */
      const seam = process.env.LOCUST_TEST_PICK_FOLDER
      if (seam !== undefined && seam.length > 0) {
        const chosen = (await readFile(seam, 'utf8').catch(() => '')).trim()
        return chosen.length > 0 ? chosen : undefined
      }
      const owner = BrowserWindow.fromWebContents(event.sender)
      const options: Electron.OpenDialogOptions = {
        title: purpose === 'restore' ? 'Choose a Locust backup to restore' : 'Choose where to put the backup',
        buttonLabel: purpose === 'restore' ? 'Restore from here' : 'Back up here',
        properties: ['openDirectory', 'createDirectory']
      }
      const picked = owner === null ? await dialog.showOpenDialog(options) : await dialog.showOpenDialog(owner, options)
      return picked.canceled ? undefined : picked.filePaths[0]
    })
    ipcMain.handle(PROFILE_BACKUP_CHANNEL, async (event, folder: unknown) => {
      const parent = profileFolderOf(folder)
      if (!fromOwnWindow(event) || parent === undefined) return { ok: false, reason: 'The backup was refused.' } as const
      return writeBackup(app.getPath('userData'), parent, { appVersion: app.getVersion(), now: () => new Date() })
    })
    ipcMain.handle(PROFILE_RESTORE_PREVIEW_CHANNEL, async (event, folder: unknown) => {
      const from = profileFolderOf(folder)
      if (!fromOwnWindow(event) || from === undefined) return { ok: false, reason: 'The restore was refused.' } as const
      const reading = await readBackup(from, { appVersion: app.getVersion() })
      if (!reading.ok) return reading
      return {
        ok: true,
        folder: from,
        appVersion: reading.manifest.appVersion,
        createdAt: reading.manifest.createdAt,
        counts: reading.manifest.counts,
        bytes: reading.bytes,
        current: await countProfile(app.getPath('userData'))
      } as const
    })
    ipcMain.handle(PROFILE_RESTORE_CHANNEL, async (event, folder: unknown) => {
      const from = profileFolderOf(folder)
      if (!fromOwnWindow(event) || from === undefined) return { ok: false, reason: 'The restore was refused.' } as const
      const liveRuns = new Set([...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()]).size
      const asked = await requestRestore(from, app.getPath('userData'), { appVersion: app.getVersion(), liveRuns, now: () => new Date() })
      if (!asked.ok) return asked
      note('restore', `asked for, from ${from}; restarting to apply it`)
      // After the answer reaches the window: it says "restarting" before it goes.
      setTimeout(() => {
        app.relaunch()
        app.exit(0)
      }, 600)
      return { ok: true } as const
    })
    ipcMain.handle(PROFILE_LAST_RESTORE_CHANNEL, async (event) => (fromOwnWindow(event) ? takeLastRestore(app.getPath('userData')) : undefined))

    /**
     * Missions deleted in this session, by id, so a record that returns can
     * be recognised rather than argued about. In memory only -- it answers a
     * question about one run of the app, and a list of ids surviving restart
     * would be a record of deletions, which is the opposite of the point.
     */
    const deletedThisSession = new Set<string>()

    ipcMain.handle(MISSION_DELETE_CHANNEL, async (event, missionId: unknown) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The deletion was rejected.' } } as const
      }
      const response = await deleteMissionRecord(
        missionLedger,
        missionId,
        (id) => codexMissions.hasMission(id) || antigravityMissions.hasMission(id)
      )
      /*
       * Ownership is NOT dropped here any more.
       *
       * It used to be, so the roster could never list a mission that no
       * longer existed -- and that still holds, because the roster is built
       * from the missions the ledger lists and a deleted one is not among
       * them. What changed is that the record is now kept until the trash is
       * emptied, and a restore has to bring the conversation back exactly as
       * it was, owner included. The owner is not in the ledger; it is in
       * `teammates.json`. Dropping it at delete would make every restored
       * conversation ownerless, which is a quieter kind of loss than the one
       * the trash exists to prevent. It is dropped by `emptyTrash` instead.
       */
      if (response.ok && typeof missionId === 'string') {
        deletedThisSession.add(missionId)
        note('mission-deleted', missionId)
      }
      return response
    })

    ipcMain.handle(MISSION_TRASH_LIST_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'STORAGE_UNAVAILABLE', message: 'The trash could not be read. Nothing in it has been deleted for good.' } } as const
      }
      try {
        return { ok: true, data: { missions: await missionLedger.listTrashedMissions() } } as const
      } catch {
        return { ok: false, error: { code: 'STORAGE_UNAVAILABLE', message: 'The trash could not be read. Nothing in it has been deleted for good.' } } as const
      }
    })

    ipcMain.handle(MISSION_RESTORE_CHANNEL, async (event, missionId: unknown) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The restore was rejected.' } } as const
      }
      if (typeof missionId !== 'string' || missionId.length === 0) {
        return { ok: false, error: { code: 'RESTORE_REFUSED', message: 'That conversation could not be put back. It is still in the trash.' } } as const
      }
      try {
        const back = await missionLedger.restoreMission(missionId)
        if (!back) {
          return {
            ok: false,
            error: {
              code: 'RESTORE_REFUSED',
              message: 'That conversation is not in the trash, or one with the same id is live again.'
            }
          } as const
        }
        deletedThisSession.delete(missionId)
        note('mission-restored', missionId)
        return { ok: true, data: { count: 1 } } as const
      } catch {
        return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'That conversation could not be put back. It is still in the trash.' } } as const
      }
    })

    ipcMain.handle(MISSION_TRASH_EMPTY_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) {
        return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The trash was not emptied. Everything in it is still there.' } } as const
      }
      try {
        // The owners go WITH the records, and only now: until this point a
        // restore had to be able to put the conversation back whole.
        const held = await missionLedger.listTrashedMissions()
        const gone = await missionLedger.emptyTrash()
        await references.forget(held.map((entry) => entry.missionId))
        for (const entry of held) {
          await teammates.unassignMission(entry.missionId).catch(() => undefined)
        }
        // A comparison whose every answer is gone for good goes too, with its copies (0.445).
        const trashed = new Set(held.map((entry) => entry.missionId))
        for (const compare of comparesGoneWith(await compares.list().catch(() => [] as readonly PublicCompare[]), trashed)) {
          await compares.remove(compare.compareId).catch(() => undefined)
          await discardCompareTrees(compare)
          await removeCompareCopies(compare.compareId).catch(() => undefined)
        }
        note('trash-emptied', String(gone))
        return { ok: true, data: { count: gone } } as const
      } catch {
        return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The trash was not emptied. Everything in it is still there.' } } as const
      }
    })

    // The window and authenticated MCP clients use the SAME start path,
    // including workspace, runtime, spend, owner and continuation checks.
    const startMission = async (request: unknown, emit: (update: CodexMissionUpdate) => void, startedBy?: MissionStarter) => {
      const refused = noWorkspaceRefusal()
      if (refused !== undefined) return refused
      const payload = (typeof request === 'object' && request !== null ? request : {}) as Partial<CodexMissionStartRequest>
      const prompt = payload.prompt
      // Anything but an explicit accept-edits is read-only. A malformed or
      // missing mode must never widen what a run may touch. `auto` is passed
      // through as itself and refused further in if the workspace has it
      // switched off -- one check, on the path every run takes, rather than
      // one here and another the relay could walk around.
      // `plan` travels as itself. It used to collapse into `ask` here, which
      // was harmless for PERMISSION -- both are read-only, and the sandbox is
      // decided from this same value -- but it meant the host, and therefore
      // the record, never knew a plan had been asked for. A plan reopened
      // after a restart came back as an ordinary read-only run and lost its
      // "Build this plan" offer (QA, 2026-09-06). Recording the mode was not
      // enough on its own: the word was already gone by the time anything
      // wrote it down, which is what a live drive found and the unit tests
      // could not.
      // A question to Scout (0.828): always in Ask, which is what holds a sealed turn on every runtime but OpenCode.
      const askingScout = typeof payload.teammateId === 'string' && payload.teammateId === await scout?.teammateId().catch(() => undefined)
      const mode = askingScout ? 'ask' : parsedMissionMode(payload.mode)
      /*
       * Validated BEFORE the peer context is built, because building it has
       * a side effect: a teammate set to work on its own branch gets its
       * worktree made by `ensure`. A start that is about to be refused must
       * not leave a branch behind, and every path below needs a prompt, so
       * the one check belongs here rather than in three of them.
       */
      if (typeof prompt !== 'string' || prompt.trim().length === 0) {
        return { ok: false, error: { code: 'INVALID_PROMPT', message: 'Enter a mission first.' } } as const
      }
      // Resolve before choosing a transport: discovery in the renderer may
      // still be partial, but the host already knows this teammate's route.
      const savedTeammate = (await teammates.list().catch(() => [])).find((entry) => entry.teammateId === payload.teammateId)
      const { runtime, model, effort, account } = routeAtStart(payload, savedTeammate)
      const managed = policyTurnRefusal(organizationPolicy, { runtime, model, mode, cloud: payload.cloud === true })
      if (managed !== undefined) return { ok: false, error: { code: 'POLICY_REFUSED', message: managed } } as const
      const peer = await peerContextFor(payload.teammateId)
      /*
       * A QUESTION TO SCOUT (0.828, shared/scout-log.ts): the person's own turn with Scout's teammate is answered from
       * What Scout saw, and sealed as its looks are -- no tool, no file, no command -- since the log quotes other
       * people's words. Its own slot, so a look running beside it never holds it up.
       */
      const scoutSlot = askingScout && scout !== undefined
        ? { key: `scout-chat:${String(payload.teammateId)}`, sealed: true as const, instructions: await scout.whyBrief() }
        : undefined
      const startingIssue = startedIssue((payload as { issue?: unknown }).issue)
      if (startingIssue !== undefined && peer?.cwd !== undefined && peer.repositoryRoot !== undefined) {
        await checkoutIssueBranch((args, cwd) => gitText(ownGitArgs(args), cwd), peer.cwd, startingIssue.number, startingIssue.title).catch(() => undefined)
      }
      // An agent being updated is being replaced on disk: a run started on
      // it now would start on half of one (runtime-updates.ts).
      if (runtimeUpdates.updating() === runtime) {
        return {
          ok: false,
          error: {
            code: 'RUNTIME_START_FAILED',
            message: `${runtimeDisplayName(runtime)} is updating to its newest version. Try again in a moment.`
          }
        } as const
      }
      // `approve-each` needs a runtime able to stop and ask, and only Codex
      // has one. Another runtime asked for it would have been started on
      // Codex without a word; it is refused instead.
      if (runtime === 'antigravity' && antigravityProbe.cliPath() === undefined) {
        // The prompt was checked above, before the peer context was built.
        if (mode !== 'accept-edits') {
          return {
            ok: false,
            error: {
              code: 'RUNTIME_START_FAILED',
              message: 'Without Antigravity CLI, Antigravity runs only in Edit: Locust cannot hold the app read-only. Install Antigravity CLI (Settings > AI agents) to run it in any mode, or choose Edit.'
            }
          } as const
        }
        const followUpOf = typeof payload.followUpOf === 'string' ? payload.followUpOf : undefined
        // Antigravity continues only its own conversation, so it cannot start again partway through one (0.498).
        if (rewindTip(payload) !== undefined && followUpOf !== undefined) {
          return {
            ok: false,
            error: { code: 'RUNTIME_START_FAILED', message: 'Antigravity cannot start again from an earlier message: it only carries on its own conversation. Pick another route for the edited message, or edit the first message to start over.' }
          } as const
        }
        try {
          const mission = await antigravityMissions.start(prompt, peer, {
            ...(model === undefined ? {} : { model }),
            ...(followUpOf === undefined ? {} : { followUpOf })
          })
          await assignOwner(peer?.self.teammateId, mission.missionId)
          await rememberRoute(peer?.self.teammateId, { runtime: 'antigravity', model: mission.model, mode })
          await advanceHub(peer?.self.teammateId, rewindTip(payload) ?? followUpOf, mission.missionId)
          return { ok: true, data: antigravityStartData(mission) } as const
        } catch (error) {
          if (error instanceof PeerRecordError) {
            return { ok: false, error: { code: 'PERSISTENCE_FAILED', message: error.message } } as const
          }
          if (error instanceof AntigravityStartError && error.busy === 'limit') {
            return { ok: false, error: { code: 'SPEND_LIMIT_REACHED', message: error.message } } as const
          }
          return {
            ok: false,
            error: {
              code: error instanceof AntigravityStartError ? 'RUNTIME_START_FAILED' : 'INTERNAL_ERROR',
              message: error instanceof Error ? error.message : 'Antigravity could not start the mission.'
            }
          } as const
        }
      }
      // Codex asks through its app-server, OpenCode through its own server
      // (A6.7), Copilot and the agents the person added through the Agent Client Protocol (0.377, 0.777),
      // Claude Code through Locust's permission host, in its own default mode (0.837).
      if (mode === 'approve-each' && runtime !== 'codex' && runtime !== 'opencode' && runtime !== 'copilot' && runtime !== 'acp' && runtime !== 'claude') {
        return {
          ok: false,
          error: {
            code: 'RUNTIME_START_FAILED',
            message: `Per-action approvals run on Claude Code, Codex CLI, OpenCode, Copilot CLI and your own agents only. Pick another mode for ${runtimeDisplayName(runtime)}, or switch the route.`
          }
        } as const
      }
      try {
        if (peer?.worktreeRefused !== undefined) {
          return { ok: false, error: { code: 'RUNTIME_START_FAILED', message: peer.worktreeRefused } } as const
        }
        const followUpOf = typeof payload.followUpOf === 'string' ? payload.followUpOf : undefined
        const response = await codexMissions.start(
          prompt,
          runtime,
          mode,
          { ...(model === undefined ? {} : { model }), ...(effort === undefined ? {} : { effort }), ...(account === undefined ? {} : { account }) },
          emit,
          undefined,
          peer,
          followUpOf,
          undefined,
          startedBy,
          // The person's own message, typed as one of the runtime's commands (0.426).
          runtimeCommands.isCommand(runtime, prompt),
          scoutSlot,
          undefined,
          rewindTip(payload) !== undefined && followUpOf !== undefined,
          // A reply on another runtime: what the person chose to leave out of its brief (0.527).
          chosenLeaveOut((payload as { readonly leaveOut?: unknown }).leaveOut),
          // Your cloud computer (0.761): only the person's own Cloud, and Set it up when they pressed it.
          (payload as { readonly cloud?: unknown }).cloud === true
            ? { setUp: (payload as { readonly cloudSetUp?: unknown }).cloudSetUp === true }
            : undefined,
          payload.referenceIds
        )
        if (response.ok) {
          // As it was resolved (0.812): a fix of this turn runs exactly as it did.
          rememberRouteOfTurn(response.data.missionId, { runtime, ...(model === undefined ? {} : { model }), ...(effort === undefined ? {} : { effort }), ...(account === undefined ? {} : { account }), mode })
          await assignOwner(peer?.self.teammateId, response.data.missionId)
          // A review's read-only run is for that run alone (0.514): the saved route stays the teammate's.
          if ((payload as { readonly keepSavedRoute?: unknown }).keepSavedRoute !== true) {
            await rememberRoute(peer?.self.teammateId, { runtime, model: model ?? 'account-default', mode, ...(effort === undefined ? {} : { effort }), ...(account === undefined ? {} : { account }) })
          }
          // A rewind moves the teammate's conversation from the old tip to the new branch (0.498).
          await advanceHub(peer?.self.teammateId, rewindTip(payload) ?? followUpOf, response.data.missionId)
          // Only a rewind partway through: editing the first message keeps the old conversation as it was.
          if (rewindTip(payload) !== undefined && followUpOf !== undefined) await retireSetAside(payload)
        }
        return response
      } catch {
        return {
          ok: false,
          error: {
            code: 'INTERNAL_ERROR',
            message: 'The Codex mission could not be started.'
          }
        } as const
      }
    }
    ipcMain.handle(CODEX_MISSION_START_CHANNEL, async (event, request: unknown) => {
      const owner = BrowserWindow.fromWebContents(event.sender)
      if (!owner || !event.senderFrame || event.senderFrame.parent !== null) {
        return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The Codex mission request was rejected.' } } as const
      }
      /*
       * A NEW CONVERSATION IN A FOLDER LIKE HOME OR .claude (0.790): not started until the person says so, once per
       * folder, since a teammate reads such a folder as its project, sign-in files included. The window asks, in a
       * card over the message box (Composer.tsx); its Start here comes back through WORKSPACE_CONFIRM_CHANNEL. A
       * reply, an edited message or a cloud turn is not asked: the conversation has its folder, or works in none here.
       */
      const fresh = typeof request === 'object' && request !== null && !('followUpOf' in request) && !('rewind' in request) && (request as { readonly cloud?: unknown }).cloud !== true
      /*
       * A MESSAGE THAT NAMES ITS PROJECT STARTS THERE (0.814, named-folder.ts). A new conversation from a folder like
       * .claude, or from one checkout, whose message names a file inside a project -- another worktree of the same
       * repository, say -- moves the window to that project first, as the folder menu would, so a handoff pasted
       * anywhere works where its brief is. The conversation says where it went.
       */
      const prompt = fresh ? (request as { readonly prompt?: unknown }).prompt : undefined
      const named = typeof prompt === 'string' && workspaceChosen
        ? projectNamedIn(prompt, {
          current: workspacePath,
          isNoProject: (folder) => folderCaution(folder) !== undefined || isInstallFolder(folder),
          same: (one, other) => sameFolder(one, other, process.platform)
        })
        : undefined
      const before = workspacePath
      const movedFrom = basename(before) || before
      const movedTo = named === undefined ? undefined : await switchFolder(named).catch(() => undefined)
      const why = fresh && workspaceChosen && !folderConfirmed(workspacePath) ? folderCaution(workspacePath) : undefined
      if (why !== undefined) {
        return { ok: false, error: { code: 'FOLDER_NOT_CONFIRMED', message: `Not started: this is ${why}.`, folderCaution: why } } as const
      }
      return startMission(request, update => {
        if (!owner.isDestroyed() && !owner.webContents.isDestroyed()) owner.webContents.send(CODEX_MISSION_UPDATE_CHANNEL, update)
      }).then(async (response) => {
        // A turn the person sent from the chat box: the only kind carried on without a press when its plan runs out
        // (0.774). Not one on the cloud computer, whose session cannot be carried to another agent here.
        const cloud = typeof request === 'object' && request !== null && (request as { readonly cloud?: unknown }).cloud === true
        if (response.ok && !cloud) rememberSentByPerson(response.data.missionId)
        if (response.ok) {
          const source = request as { scoutCardId?: unknown; teammateId?: unknown }
          if (typeof source.scoutCardId === 'string') await keptScout.sentComment(source.scoutCardId, response.data.missionId, typeof source.teammateId === 'string' ? source.teammateId : undefined)
          const issue = startedIssue((request as { issue?: unknown }).issue)
          const follow = (request as { followUpOf?: unknown }).followUpOf
          if (issue !== undefined) await githubIssues.remember(response.data.missionId, issue).catch(() => undefined)
          else if (typeof follow === 'string') await githubIssues.copy(follow, response.data.missionId).catch(() => undefined)
        }
        if (response.ok && movedTo !== undefined) {
          if (!owner.isDestroyed() && !owner.webContents.isDestroyed()) {
            owner.webContents.send(CODEX_MISSION_UPDATE_CHANNEL, {
              kind: 'relay-notice',
              runId: response.data.runId,
              missionId: response.data.missionId,
              message: `Working in ${movedTo.name}, the project your message names, instead of ${movedFrom}.`,
              level: 'info'
            })
          }
          return { ...response, data: { ...response.data, movedToFolder: { path: movedTo.path, name: movedTo.name } } }
        }
        // Nothing started, so nothing moved: the window goes back to the folder it was in, words and all.
        if (!response.ok && movedTo !== undefined) await switchFolder(before).catch(() => undefined)
        return response
      })
    })
    /*
     * FIX UNTIL THE CHECK PASSES (0.812, check-fix.ts). A fix starts through the same path a person's message takes --
     * workspace, policy, spend, owner and continuation checks all apply -- as the next turn of the same conversation,
     * on the route the failing turn ran on, recorded as Locust's (`check-fix`), never as the person's words.
     */
    const noteCheckFix = oneAtATime(async ({ missionId, message, level }: { readonly missionId: string; readonly message: string; readonly level: 'info' | 'warning' }) => {
      const mission = await missionLedger.getMission(missionId).catch(() => undefined)
      if (mission === undefined) return
      sendToWindow({ kind: 'relay-notice', runId: mission.metadata.runId, missionId, message, level })
      const last = mission.events.at(-1)
      const sequence = (last?.sequence ?? 0) + 1
      await missionLedger
        .appendEvents(missionId, [
          {
            id: `${mission.metadata.runId}:check-fix:${String(sequence)}`,
            runId: mission.metadata.runId,
            missionId,
            sequence,
            type: 'adapter.diagnostic',
            occurredAt: new Date().toISOString(),
            sourceAdapter: last?.sourceAdapter ?? mission.metadata.runtime,
            payload: { level, code: 'host.check_fix', message, terminal: false, evidence: { redacted: true } }
          } as NormalizedRuntimeEvent
        ])
        .catch(() => undefined)
    })
    checkFix = createCheckFix({
      // The turn's own folder's setting (0.813, Sol's review), not the folder the window has switched to since.
      mostFixes: async (workspaceId) => {
        const folder = workspaceId ?? (workspaceChosen ? workspaceIdFor(workspacePath) : undefined)
        return folder === undefined ? undefined : parsedCheckFixTries((await teammates.readSettings()).checkFixes?.[folder])
      },
      sentByPerson: (missionId) => sentByPerson.has(missionId),
      // The person got there first: a newer turn in this conversation, or its teammate already at work.
      busy: async (turn) =>
        (await newestTurnOf(missionLedger, turn.missionId).catch(() => turn.missionId)) !== turn.missionId
        || (turn.teammateId !== undefined && codexMissions.runIdOwnedBy(turn.teammateId) !== undefined),
      nameOf: async (teammateId) => (teammateId === undefined ? undefined : (await teammates.list().catch(() => [])).find((entry) => entry.teammateId === teammateId)?.name) ?? 'the teammate',
      note: (missionId, message, level) => void noteCheckFix({ missionId, message, level }).catch(() => undefined),
      start: async ({ followUpOf, teammateId, prompt, startedBy }) => {
        // The failing turn's own route: as it was resolved when it started, else as its record has it.
        const held = routeOfTurn.get(followUpOf)
        const recorded = held === undefined ? await missionLedger.getMission(followUpOf).catch(() => undefined) : undefined
        const recordedAccount = recorded === undefined ? undefined : accountFromRoute(recorded.metadata.resolvedRouteId)
        const route = held ?? (recorded === undefined ? undefined : {
          runtime: recorded.metadata.runtime,
          ...(recorded.metadata.model === 'account-default' ? {} : { model: recorded.metadata.model }),
          ...(recordedAccount === undefined ? {} : { account: recordedAccount }),
          mode: recorded.metadata.mode ?? 'accept-edits'
        })
        if (route === undefined) return { ok: false, message: 'The turn it would fix is not in the record any more.' }
        const owners = await teammates.missionOwners().catch(() => ({} as Readonly<Record<string, string>>))
        const wasHub = teammateId !== undefined && (await teammates.list().catch(() => [])).find((entry) => entry.teammateId === teammateId)?.hubMissionId === followUpOf
        const response = await startMission({
          prompt,
          ...(teammateId === undefined ? {} : { teammateId, routeOverrideFor: teammateId }),
          followUpOf,
          runtime: route.runtime,
          ...(route.model === undefined ? {} : { model: route.model }),
          ...(route.effort === undefined ? {} : { effort: route.effort }),
          ...(route.account === undefined ? {} : { account: route.account }),
          mode: route.mode,
          // A fix is not the person picking a route: the teammate's saved one stays as it is.
          keepSavedRoute: true
        } as CodexMissionStartRequest, sendToWindow, startedBy)
        // Refused because the conversation went on or is answering: the person's, not a failure (0.813). The pool's
        // own limit is a failure to say.
        if (!response.ok) return { ok: false, message: response.error.message, ...(response.error.code === 'RUN_ALREADY_ACTIVE' && response.error.busy !== 'pool' ? { busy: true as const } : {}) }
        const owner = owners[followUpOf] ?? teammateId
        sendToWindow({
          kind: 'mission-started',
          runId: response.data.runId,
          missionId: response.data.missionId,
          ...(owner === undefined ? {} : { teammateId: owner }),
          prompt,
          data: response.data,
          startedBy,
          ...(wasHub ? { hubMissionId: response.data.missionId } : {})
        })
        return { ok: true, missionId: response.data.missionId }
      }
    })
    /*
     * The conversations an app may list (0.842): the newest turn of each, in the open folder, as the window lists them --
     * never a panel's own run (a commit message, a walkthrough) -- at most 20, newest first.
     */
    const appConversations = async (): Promise<readonly PublicRecoveredMission[]> => {
      if (!workspaceChosen) return []
      const history = await readMissionHistory(missionLedger, undefined, workspacePath)
      if (!history.ok) return []
      const hidden = new Set(await panelRuns.list().catch(() => [] as readonly string[]))
      const newest: PublicRecoveredMission[] = []
      for (const turn of [...history.data.missions].sort((a, b) => b.lastUpdatedAt.localeCompare(a.lastUpdatedAt))) {
        if (newest.length >= 20) break
        if (turn.workspaceId !== history.data.currentWorkspaceId || hidden.has(turn.missionId)) continue
        if (await newestTurnOf(missionLedger, turn.missionId) === turn.missionId) newest.push(turn)
      }
      return newest
    }
    const mcpTools = createLocustMcpTools({
      teammates: listTeammates,
      busy: id => codexMissions.runIdOwnedBy(id) !== undefined || antigravityMissions.runIdOwnedBy(id) !== undefined,
      start: async (request, origin) => {
        const response = await startMission(request, sendToWindow, origin)
        if (response.ok) sendToWindow({ kind: 'mission-started', runId: response.data.runId, missionId: response.data.missionId,
          teammateId: request.teammateId, prompt: request.prompt, data: response.data, startedBy: { kind: 'mcp' } })
        return response
      },
      read: id => readOneMission(missionLedger, workroom, id),
      newest: id => newestTurnOf(missionLedger, id),
      owner: async id => (await teammates.missionOwners())[id],
      live: id => codexMissions.hasMission(id) || antigravityMissions.liveMissionIds().includes(id),
      background: async () => (await backgroundRuns.list()).map(publicRun),
      // Called per tool call, after mcpHost below exists.
      ownMode: () => mcpHost.ownMode(),
      waiting: id => raisedApprovals.waitingOn(id),
      // The routine card's own Run path, with the same folder check (0.704).
      routines: async () => { await routineRunner?.reconcile(); return routines.list() },
      runRoutine: (id, values) => workspaceChosen ? routineIO.run(id, values) : Promise.resolve(routineRejected(NO_WORKSPACE_MESSAGE)),
      routineSteps: () => routineRunner?.running() ?? [],
      routineLastStep: id => routineLastStep.get(id),
      // A model and effort another app asks for, checked against the window's own list (0.842).
      models: async () => { const read = await readModelCatalog(); return read.ok ? read.data.models : [] },
      // Where every turn works, by the folder's name alone: never the path above it (0.842).
      folder: () => (workspaceChosen ? basename(workspacePath) : undefined),
      // The window's own Stop, for a turn an app started (0.842).
      cancel: runId => codexMissions.cancel(runId).ok || antigravityMissions.cancel(runId),
      // The person's own conversations too, only while their switch is on (0.842).
      seeAll: () => mcpHost.conversations(),
      conversations: () => appConversations(),
      carryOn: (missionId, route) => carryOn?.carry(missionId, route) ?? Promise.resolve({ ok: false, message: 'Locust is still starting.' })
    })
    const cliSessions = createLocustCliSessions({
      teammates: async () => {
        const list = await listTeammates()
        if (!list.ok) throw new Error(list.error.message)
        return list.data.teammates.map(t => ({ id: t.teammateId, name: t.name, runtime: t.route?.runtime ?? null,
          model: t.route?.model ?? null, runs_in: mcpHost.ownMode() ? t.route?.mode ?? 'ask' : 'ask' }))
      },
      workspace: () => workspaceChosen ? workspacePath : undefined,
      start: async (request, update) => {
        const response = await startMission(request, event => { sendToWindow(event); update(event) }, { kind: 'mcp' })
        if (response.ok) sendToWindow({ kind: 'mission-started', runId: response.data.runId, missionId: response.data.missionId,
          teammateId: request.teammateId, prompt: request.prompt, data: response.data, startedBy: { kind: 'mcp' } })
        return response
      },
      cancel: runId => { if (!codexMissions.cancel(runId).ok) antigravityMissions.cancel(runId) },
      approvals: () => raisedApprovals.waiting().map(a => a.request),
      // Off until the person turns on Let editors answer cards for their own turns (0.790): cards wait in the window.
      editorAnswers: () => mcpHost.editorAnswers(),
      opened: () => { void mcpHost.noteEditor() },
      // Only cliSessions' capability-checked current turn reaches this, never a tool in the MCP bridge.
      answer: async (approvalId, approve) => {
        const answered = await decideApproval({ approvalId, decision: approve ? 'approve-once' : 'deny' })
        const target = approvalWindow
        if (answered && target && !target.isDestroyed() && !target.webContents.isDestroyed()) target.webContents.send(MISSION_APPROVAL_WITHDRAWN_CHANNEL, approvalId)
        return answered
      },
      status: async () => {
        const history = await readMissionHistory(missionLedger, undefined, workspacePath)
        if (!history.ok) throw new Error(history.error.message)
        const owners = await teammates.missionOwners()
        const team = await teammates.list()
        return { workspace: workspacePath, turns: history.data.missions.slice(0, 50).map(mission => ({
          missionId: mission.missionId,
          teammate: team.find(t => t.teammateId === owners[mission.missionId])?.name ?? 'A teammate no longer here',
          state: raisedApprovals.waitingOn(mission.missionId) ? 'waiting' as const
            : codexMissions.hasMission(mission.missionId) || antigravityMissions.liveMissionIds().includes(mission.missionId) ? 'running' as const : mission.phase
        })) }
      }
    })
    const mcpHost = createLocustMcpHost({ directory: app.getPath('userData'), node: process.execPath,
      refusal: () => policyFeatureRefusal(organizationPolicy, 'otherApps'),
      bridge: app.isPackaged ? join(process.resourcesPath, 'locust-mcp-bridge.mjs') : join(__dirname, '../../resources/locust-mcp-bridge.mjs'),
      call: (name, args) => name.startsWith('cli/') ? cliSessions.call(name, args) : mcpTools(name, args) })
    app.once('will-quit', () => cliSessions.dispose())
    /*
     * THE CALL'S COPY (0.739): what a voice call's Codex hands work over with. The same tools another app gets,
     * but a turn runs in the teammate's own mode -- the person asked for it aloud, in Locust, and heard it read
     * back -- and is recorded as handed over in a voice call. Only the hand-off tools answer; its listener is its
     * own, with its own token, started for a call and stopped with it, so the switch for other apps stays as set.
     */
    const voiceCallTools = createLocustMcpTools({
      teammates: listTeammates,
      busy: id => codexMissions.runIdOwnedBy(id) !== undefined || antigravityMissions.runIdOwnedBy(id) !== undefined,
      start: async (request, origin) => {
        const response = await startMission(request, sendToWindow, origin)
        if (response.ok) sendToWindow({ kind: 'mission-started', runId: response.data.runId, missionId: response.data.missionId,
          teammateId: request.teammateId, prompt: request.prompt, data: response.data, startedBy: { kind: 'mcp', via: 'voice' } })
        return response
      },
      read: id => readOneMission(missionLedger, workroom, id),
      newest: id => newestTurnOf(missionLedger, id),
      owner: async id => (await teammates.missionOwners())[id],
      live: id => codexMissions.hasMission(id) || antigravityMissions.liveMissionIds().includes(id),
      background: async () => (await backgroundRuns.list()).map(publicRun),
      ownMode: () => true,
      origin: { kind: 'mcp', via: 'voice' },
      waiting: id => raisedApprovals.waitingOn(id),
      // The one conversation a call with a teammate may continue, and only while that call is on (0.746).
      here: () => voiceCallHere?.conversationId
    })
    // The call reads the folder with Locust's own read-only tools, never a command (0.742).
    const voiceCallFolder = callFolderTools(() => (workspaceChosen ? workspacePath : undefined))
    const voiceCallBridge = app.isPackaged ? join(process.resourcesPath, 'locust-mcp-bridge.mjs') : join(__dirname, '../../resources/locust-mcp-bridge.mjs')
    const voiceCallMcpDirectory = join(app.getPath('userData'), 'voice-call-mcp')
    const voiceCallHost = createLocustMcpHost({ directory: voiceCallMcpDirectory, node: process.execPath, bridge: voiceCallBridge,
      call: (name, args) => !VOICE_CALL_TOOLS.has(name)
        ? Promise.resolve({ content: [{ type: 'text', text: 'A voice call can only read the folder, hand work to teammates and read their replies.' }], isError: true })
        : CALL_FOLDER_TOOLS.has(name) ? voiceCallFolder(name, args) : voiceCallTools(name, args) })
    voiceCallAddOn = {
      open: async () => {
        const state = await voiceCallHost.setEnabled(true)
        if (!state.enabled) return undefined
        return { command: process.execPath, args: [voiceCallBridge, join(voiceCallMcpDirectory, MCP_CONNECTION_FILE), 'call'], env: { ELECTRON_RUN_AS_NODE: '1' } }
      },
      close: async () => { await voiceCallHost.setEnabled(false) }
    }
    app.once('will-quit', () => { void voiceCallHost.dispose() })
    mcpForShutdown = mcpHost
    void mcpHost.load().catch(() => note('locust-mcp', 'The local MCP listener could not start. It is off.'))
    /*
     * YOUR PHONE (0.775, shared/phone.ts). What the phone shows is what the window's history shows; a card it answers
     * goes down the window's own path (decideApproval); a message it sends is the person's next turn, started as the
     * MCP tools start one -- on the teammate's own route and mode -- and recorded as the person's, because it is.
     */
    const phoneNames = async () => {
      const owners = await teammates.missionOwners().catch(() => ({} as Readonly<Record<string, string>>))
      const listed = await teammates.list().catch(() => [])
      return (missionId: string) => listed.find((mate) => mate.teammateId === owners[missionId])?.name
    }
    const phoneLive = (): string[] => [...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()]
    /*
     * Built once per change, not once per poll: an open phone page asks every 3 s, and reading the whole history
     * each time cost 131 ms at 2,000 missions (review 2026-10-15, P4). The key is the ledger's write count, what is
     * live, and the minute, so a teammate renamed in Settings reaches the phone within one.
     */
    const phoneHistoryOnce = keyedOnce(async () => {
      const history = await readMissionHistory(missionLedger, undefined, workspacePath)
      const live = phoneLive()
      const names = await phoneNames()
      // Scout's own looks are never a conversation, on the phone as in the window (0.829: the away drive's phone listed one).
      const looks = new Set((await keptScout.state().catch(() => undefined))?.lookIds ?? [])
      const missions = (history.ok ? history.data.missions : []).filter((mission) => !looks.has(mission.missionId))
      return { ...phoneConversations(missions, live, names), live: live.filter((missionId) => !looks.has(missionId)), names }
    })
    const phoneHistory = () => phoneHistoryOnce(`${String(ledgerWrites())}|${phoneLive().join(',')}|${String(Math.floor(Date.now() / 60_000))}`)
    const phoneSending = new Set<string>()
    /** A phone's message as the person's next turn, and, when it was not sent, whether it may be tried again (0.837). */
    const phoneSend = async (missionId: string, text: string): Promise<{ readonly ok: true } | { readonly ok: false; readonly message: string; readonly why: 'busy' | 'gone' | 'failed' }> => {
      const newest = await newestTurnOf(missionLedger, missionId)
      // Two presses, or the phone and the window at once, must not both continue the same turn.
      if (phoneSending.has(newest)) return { ok: false, message: 'That message is already on its way.', why: 'busy' }
      if (codexMissions.hasMission(newest) || antigravityMissions.liveMissionIds().includes(newest)) {
        return { ok: false, message: 'This conversation is still working. Send when it finishes.', why: 'busy' }
      }
      phoneSending.add(newest)
      try {
        const owner = (await teammates.missionOwners().catch(() => ({} as Readonly<Record<string, string>>)))[newest]
        if (owner === undefined) return { ok: false, message: 'This conversation no longer has a teammate in Locust. Nothing was sent.', why: 'gone' }
        const list = await listTeammates()
        const mode = (list.ok ? list.data.teammates.find((mate) => mate.teammateId === owner)?.route?.mode : undefined) ?? 'ask'
        const response = await startMission({ prompt: text, teammateId: owner, followUpOf: newest, mode, keepSavedRoute: true } as CodexMissionStartRequest, sendToWindow)
        if (!response.ok) return { ok: false, message: response.error.message, why: 'failed' }
        rememberSentByPerson(response.data.missionId)
        sendToWindow({ kind: 'mission-started', runId: response.data.runId, missionId: response.data.missionId, teammateId: owner, prompt: text, data: response.data })
        return { ok: true }
      } finally {
        phoneSending.delete(newest)
      }
    }
    // What a phone may see and do, the same on the home network and from anywhere (0.826).
    const phoneCalls: PhoneCalls = {
      state: async () => ({
        waiting: await Promise.all(raisedApprovals.waiting().map(async ({ request, teammateId }) => {
          const name = await scoutNameOf(teammateId)
          // What is allowed is said here as in the chat apps (0.840): "Run a command" alone was Allow once on nothing.
          const exact = request.kind === 'question' ? undefined : exactLine(request)
          const plain = request.kind === 'command' ? plainCommand(request.detail) : undefined
          return {
            approvalId: request.approvalId, missionId: request.missionId, summary: request.summary, requestedAt: request.requestedAt, ...(name === undefined ? {} : { name }),
            ...(exact === undefined ? {} : { exact }), ...(plain === undefined ? {} : { plain: { said: scrubSecrets(plain.said).text, careful: plain.careful } })
          }
        })),
        conversations: (await phoneHistory()).list,
        scout: phoneScoutCards((await keptScout.state().catch(() => undefined))?.cards ?? [])
      }),
      dismissScout: async (cardId) => {
        const before = (await keptScout.state()).cards.some((card) => card.cardId === cardId)
        if (!before) return false
        await keptScout.dismiss(cardId)
        return true
      },
      turns: async (missionId) => {
        const { chains, live, names } = await phoneHistory()
        return phoneTurns(chains, missionId, live, names)
      },
      decide: async (approvalId, approve) => {
        const answered = await decideApproval({ approvalId, decision: approve ? 'approve-once' : 'deny' })
        // The window's card comes down, as one answered in Antigravity's own window does.
        const target = approvalWindow
        if (answered && target && !target.isDestroyed() && !target.webContents.isDestroyed()) target.webContents.send(MISSION_APPROVAL_WITHDRAWN_CHANNEL, approvalId)
        return answered
      },
      send: async (missionId, text) => {
        const sent = await phoneSend(missionId, text)
        return sent.ok ? sent : { ok: false, message: sent.message }
      }
    }
    const phoneHost = createPhoneHost({
      refusal: () => policyFeatureRefusal(organizationPolicy, 'phone'),
      directory: app.getPath('userData'),
      page: phonePage,
      calls: phoneCalls
    })
    let deskLink: DeskHost | undefined
    /** Other apps from anywhere (0.841, remote-mcp.ts): made just below, once the relay's line exists. */
    let remoteMcp: ReturnType<typeof createRemoteMcp> | undefined
    const phoneAnywhere = createPhoneAnywhere({
      refusal: () => policyFeatureRefusal(organizationPolicy, 'phone'),
      directory: app.getPath('userData'),
      calls: phoneCalls,
      // Locust's own relay, unless LOCUST_RELAY names another copy: a person's own, or one a drive runs locally.
      relay: () => {
        const named = process.env.LOCUST_RELAY?.trim()
        return named !== undefined && /^https?:\/\/[^\s/]+$/.test(named) ? named : DEFAULT_RELAY
      },
      changed: (settings) => {
        if (approvalWindow !== undefined && !approvalWindow.isDestroyed() && !approvalWindow.webContents.isDestroyed()) approvalWindow.webContents.send(PHONE_ANYWHERE_CHANGED_CHANNEL, settings)
        scoutAway.nudge()
        // From anywhere off forgets the room, and with it every app's address: they are let go here too.
        if (!settings.enabled && remoteMcp?.settings().enabled === true) void remoteMcp.setEnabled(false)
      },
      mcp: {
        answer: (asked) => remoteMcp?.answer(asked) ?? Promise.resolve({ status: 503, body: JSON.stringify({ error: 'pc-offline' }) }),
        sync: () => { void remoteMcp?.sync() }
      },
      away: { forPhone: () => scoutAway.forPhone(), refresh: () => scoutAway.refresh(), letter: (letter) => scoutAway.letter(letter) },
      hooks: { refresh: syncHooks },
      // Bound once the desk host is made, a few lines on; a frame before then waits for nothing.
      desk: { frame: (device, asks) => { void deskLink?.handle(device, asks) }, forget: (device) => { void deskLink?.forget(device) } },
      // Written on a phone while this computer was off (0.837): sent now, through the phone's own path.
      notes: { deliver: async (missionId, text) => { const sent = await phoneSend(missionId, text); return sent.ok ? 'sent' : sent.why } },
      // Signed in with GitHub (0.838): GitHub's own answer, unless LOCUST_GITHUB_API names a stand-in a drive runs.
      githubUser: (token) => {
        const named = process.env.LOCUST_GITHUB_API?.trim()
        return githubUserOf(token, named !== undefined && /^https?:\/\/[^\s/]+$/.test(named) ? named : undefined)
      },
      computerName: () => hostname()
    })
    hookRelay = phoneAnywhere
    remoteMcp = createRemoteMcp({
      file: join(app.getPath('userData'), REMOTE_MCP_FILE),
      // The tools an app on this computer gets, with the same rules: Ask unless each teammate's own mode is on.
      call: (name, args) => mcpTools(name, args),
      address: () => phoneAnywhere.mcpAddress(),
      open: (on) => phoneAnywhere.mcpOpen(on),
      refusal: () => policyFeatureRefusal(organizationPolicy, 'otherApps') ?? policyFeatureRefusal(organizationPolicy, 'phone'),
      changed: (state) => { if (approvalWindow !== undefined && !approvalWindow.isDestroyed() && !approvalWindow.webContents.isDestroyed()) approvalWindow.webContents.send(REMOTE_MCP_CHANGED, state) },
      asked: (name) => { attention.connectAsked(name) },
      version: app.getVersion()
    })
    void remoteMcp.load().then(() => { if (phoneAnywhere.connected()) void remoteMcp?.sync() }, () => undefined)
    ipcMain.handle(REMOTE_MCP_GET, (event) => fromOwnWindow(event) && remoteMcp !== undefined ? remoteMcp.settings() : { enabled: false, apps: [], asking: [] })
    ipcMain.handle(REMOTE_MCP_SET, (event, on: unknown) => fromOwnWindow(event) && remoteMcp !== undefined ? remoteMcp.setEnabled(on) : { enabled: false, apps: [], asking: [] })
    ipcMain.handle(REMOTE_MCP_REMOVE, (event, id: unknown) => fromOwnWindow(event) && remoteMcp !== undefined ? remoteMcp.remove(id) : { enabled: false, apps: [], asking: [] })
    // Only the window answers a card: never an app, never the relay.
    ipcMain.handle(REMOTE_MCP_DECIDE, (event, request: unknown, allow: unknown) => fromOwnWindow(event) && remoteMcp !== undefined ? remoteMcp.answerSignIn(request, allow) : { enabled: false, apps: [], asking: [] })
    app.once('will-quit', () => { remoteMcp?.dispose() })
    scoutAwayRelay = { ready: () => phoneAnywhere.settings().then(() => undefined), room: () => phoneAnywhere.room(), connected: () => phoneAnywhere.connected(), awayOpen: (hash) => phoneAnywhere.awayOpen(hash), awayClose: () => phoneAnywhere.awayClose(), keepAwayAlerts: () => phoneAnywhere.keepAwayAlerts(), post: (box) => phoneAnywhere.post(box), letters: (after) => phoneAnywhere.letters(after) }
    phoneAlertSink = (alert) => { void phoneAnywhere.alert(alert).catch(() => undefined) }
    void phoneAnywhere.settings().then((settings) => { phoneNightApprovals = settings.nightApprovals === true }, () => undefined)
    /** One way at a time: turning one on turns the other off, and off unpairs every phone of that way. */
    const setPhoneMode = async (mode: unknown): Promise<PhoneModeState> => {
      if (!PHONE_MODES.includes(mode as PhoneMode)) return { wifi: await phoneHost.settings(), anywhere: await phoneAnywhere.settings() }
      const wifi = await phoneHost.setEnabled(mode === 'wifi')
      const anywhere = await phoneAnywhere.setEnabled(mode === 'anywhere')
      return { wifi, anywhere }
    }
    const NO_PHONE: PhoneAnywhereSettings = { enabled: false, connected: false, devices: [] }
    app.once('will-quit', () => { void phoneHost.dispose(); void phoneAnywhere.dispose() })
    /*
     * YOUR TEAMMATES IN CHAT APPS (0.831 Telegram, 0.832 Discord and Slack; chat-link.ts). A message from the paired
     * chat is the person's next message to that teammate, started as the phone's is -- on the teammate's own route and
     * mode -- and recorded as theirs. The three apps share the start, the record and the window's path for a card.
     */
    const chatSending = new Set<string>()
    const chatStart = async ({ teammateId, text, followUpOf }: { readonly teammateId: string; readonly text: string; readonly followUpOf?: string }): Promise<ChatStart> => {
      const list = await listTeammates()
      const mate = list.ok ? list.data.teammates.find((one) => one.teammateId === teammateId) : undefined
      if (mate === undefined) return { ok: false, message: 'That teammate is not in Locust any more. Nothing was sent.' }
      const newest = followUpOf === undefined ? undefined : await newestTurnOf(missionLedger, followUpOf).catch(() => followUpOf)
      if (newest !== undefined) {
        // Two messages at once, or a chat and the window at once, must not both continue the same turn.
        if (chatSending.has(newest) || phoneSending.has(newest)) return { ok: false, message: 'That message is already on its way.' }
        if (codexMissions.hasMission(newest) || antigravityMissions.liveMissionIds().includes(newest)) {
          return { ok: false, message: `${mate.name} is still working on the last message. Send this one when its answer arrives.` }
        }
        chatSending.add(newest)
      }
      try {
        const response = await startMission({ prompt: text, teammateId, ...(newest === undefined ? {} : { followUpOf: newest }), mode: mate.route?.mode ?? 'ask', keepSavedRoute: true } as CodexMissionStartRequest, sendToWindow)
        if (!response.ok) return { ok: false, message: response.error.message }
        rememberSentByPerson(response.data.missionId)
        sendToWindow({ kind: 'mission-started', runId: response.data.runId, missionId: response.data.missionId, teammateId, prompt: text, data: response.data })
        return { ok: true, missionId: response.data.missionId }
      } finally {
        if (newest !== undefined) chatSending.delete(newest)
      }
    }
    const chatOptions = (changedChannel: string): ChatLinkOptions => ({
      directory: app.getPath('userData'),
      secrets: { available: () => safeStorage.isEncryptionAvailable(), encrypt: (text) => safeStorage.encryptString(text), decrypt: (cipher) => safeStorage.decryptString(cipher) },
      refusal: () => policyFeatureRefusal(organizationPolicy, 'chatApps'),
      computer: () => hostname(),
      team: async () => {
        const list = await listTeammates()
        return list.ok ? list.data.teammates.map((mate) => ({ teammateId: mate.teammateId, name: mate.name, role: mate.roleTitle ?? mate.role })) : []
      },
      start: chatStart,
      read: async (missionId) => {
        const read = await readOneMission(missionLedger, workroom, missionId)
        return read.ok ? read.data.mission : undefined
      },
      live: (missionId) => codexMissions.hasMission(missionId) || antigravityMissions.liveMissionIds().includes(missionId),
      carried: (missionId) => carryOn?.carried(missionId) === true,
      waiting: () => raisedApprovals.waiting(),
      // A chat's Allow once, Deny or answer, after the link checked the chat and its person (chat-link.ts).
      decide: async (answer) => {
        const answered = await decideApproval(answer)
        // The window's card comes down, as one answered from the phone does.
        const target = approvalWindow
        if (answered && target && !target.isDestroyed() && !target.webContents.isDestroyed()) target.webContents.send(MISSION_APPROVAL_WITHDRAWN_CHANNEL, answer.approvalId)
        return answered
      },
      changed: (settings: ChatAppSettings) => {
        if (approvalWindow !== undefined && !approvalWindow.isDestroyed() && !approvalWindow.webContents.isDestroyed()) approvalWindow.webContents.send(changedChannel, settings)
      },
      note
    })
    // Each app itself, unless a drive names its stand-in on this computer: never another host, since the token goes there.
    const standIn = (variable: string, real: string) => (): string => {
      const named = process.env[variable]?.trim()
      return named !== undefined && /^http:\/\/(?:127\.0\.0\.1|localhost):\d{1,5}$/.test(named) ? named : real
    }
    const telegramLink = createTelegramLink({ ...chatOptions(TELEGRAM_CHANGED_CHANNEL), api: standIn('LOCUST_TELEGRAM_API', TELEGRAM_API) })
    const discordLink = createDiscordLink({ ...chatOptions(DISCORD_CHANGED_CHANNEL), api: standIn('LOCUST_DISCORD_API', DISCORD_API), version: app.getVersion() })
    const slackLink = createSlackLink({ ...chatOptions(SLACK_CHANGED_CHANNEL), api: standIn('LOCUST_SLACK_API', SLACK_API) })
    chatLinks = [discordLink, slackLink, telegramLink]
    app.once('will-quit', () => { for (const link of chatLinks) void link.dispose() })
    void discordLink.load().catch(() => note('discord', 'Discord could not start. It is off.'))
    void slackLink.load().catch(() => note('slack', 'Slack could not start. It is off.'))
    void telegramLink.load().catch(() => note('telegram', 'Telegram could not start. It is off.'))
    const NO_CHAT: ChatAppSettings = { connected: false }
    ipcMain.handle(DISCORD_SETTINGS_CHANNEL, (event) => fromOwnWindow(event) ? discordLink.settings() : NO_CHAT)
    ipcMain.handle(DISCORD_SET_TOKEN_CHANNEL, (event, token: unknown) => fromOwnWindow(event) ? discordLink.setToken(token) : NO_CHAT)
    ipcMain.handle(DISCORD_NEW_LINK_CHANNEL, (event) => fromOwnWindow(event) ? discordLink.newLink() : NO_CHAT)
    ipcMain.handle(DISCORD_REMOVE_CHANNEL, (event) => fromOwnWindow(event) ? discordLink.remove() : NO_CHAT)
    ipcMain.handle(SLACK_SETTINGS_CHANNEL, (event) => fromOwnWindow(event) ? slackLink.settings() : NO_CHAT)
    ipcMain.handle(SLACK_SET_TOKEN_CHANNEL, (event, tokens: unknown) => fromOwnWindow(event) ? slackLink.setToken(tokens) : NO_CHAT)
    ipcMain.handle(SLACK_NEW_LINK_CHANNEL, (event) => fromOwnWindow(event) ? slackLink.newLink() : NO_CHAT)
    ipcMain.handle(SLACK_REMOVE_CHANNEL, (event) => fromOwnWindow(event) ? slackLink.remove() : NO_CHAT)
    ipcMain.handle(TELEGRAM_SETTINGS_CHANNEL, (event) => fromOwnWindow(event) ? telegramLink.settings() : NO_CHAT)
    ipcMain.handle(TELEGRAM_SET_TOKEN_CHANNEL, (event, token: unknown) => fromOwnWindow(event) ? telegramLink.setToken(token) : NO_CHAT)
    ipcMain.handle(TELEGRAM_NEW_LINK_CHANNEL, (event) => fromOwnWindow(event) ? telegramLink.newLink() : NO_CHAT)
    ipcMain.handle(TELEGRAM_REMOVE_CHANNEL, (event) => fromOwnWindow(event) ? telegramLink.remove() : NO_CHAT)
    ipcMain.handle(CHAT_ALERTS_CHANNEL, (event, id: unknown, on: unknown) => {
      const link = id === 'discord' ? discordLink : id === 'slack' ? slackLink : id === 'telegram' ? telegramLink : undefined
      return fromOwnWindow(event) && link !== undefined ? link.setAlerts(on) : NO_CHAT
    })
    void phoneHost.load().catch(() => note('phone', 'The page for your phone could not start. It is off.'))
    void phoneAnywhere.load().catch(() => note('phone', 'Your phone could not reach Locust from anywhere. It is off.'))
    ipcMain.handle(PHONE_SETTINGS_CHANNEL, (event) => fromOwnWindow(event) ? phoneHost.settings() : { enabled: false, addresses: [] })
    ipcMain.handle(PHONE_SET_ENABLED_CHANNEL, (event, enabled: unknown) => fromOwnWindow(event) ? phoneHost.setEnabled(enabled) : { enabled: false, addresses: [] })
    ipcMain.handle(PHONE_FORGET_CHANNEL, (event) => fromOwnWindow(event) ? phoneHost.forget() : { enabled: false, addresses: [] })
    ipcMain.handle(PHONE_ANYWHERE_CHANNEL, (event) => fromOwnWindow(event) ? phoneAnywhere.settings() : NO_PHONE)
    ipcMain.handle(PHONE_SET_MODE_CHANNEL, (event, mode: unknown) => fromOwnWindow(event) ? setPhoneMode(mode) : { wifi: { enabled: false, addresses: [] }, anywhere: NO_PHONE })
    ipcMain.handle(PHONE_NEW_CODE_CHANNEL, (event) => fromOwnWindow(event) ? phoneAnywhere.newCode() : NO_PHONE)
    ipcMain.handle(PHONE_REMOVE_CHANNEL, (event, device: unknown) => fromOwnWindow(event) ? phoneAnywhere.remove(device) : NO_PHONE)
    /*
     * WITHOUT A CODE TO CARRY (0.838). Open in your browser turns on From anywhere when phones are off, opens a code in
     * this computer's own browser, and that browser goes on to the full window. Sign in with GitHub sends the person to
     * GitHub in their browser, through Locust's relay; after that any browser where they sign in to GitHub opens Locust.
     */
    // A drive's stand-in writes the page down instead, as LOCUST_SIGN_IN_PAGES does for agents' sign-ins: a drive is not the person.
    const openForPerson = async (url: string): Promise<void> => {
      const pages = process.env.LOCUST_SIGN_IN_PAGES
      if (pages !== undefined && pages.length > 0) await appendFile(pages, `${url}
`, 'utf8')
      else await shell.openExternal(url)
    }
    const relayAnswers = async (): Promise<boolean> => {
      for (let waited = 0; waited < 15_000; waited += 250) {
        if ((await phoneAnywhere.settings()).connected) return true
        await new Promise((resolve) => setTimeout(resolve, 250))
      }
      return false
    }
    ipcMain.handle(PHONE_OPEN_BROWSER_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return NO_PHONE
      const before = await phoneAnywhere.settings()
      if (policyFeatureRefusal(organizationPolicy, 'phone') !== undefined) return before
      if (!before.enabled) {
        if ((await phoneHost.settings()).enabled) return { ...before, message: 'Your phone reaches Locust only on this Wi-Fi now. Choose From anywhere first.' }
        await setPhoneMode('anywhere')
      }
      if (!(await relayAnswers())) return { ...(await phoneAnywhere.settings()), message: 'Locust could not reach its relay. Check this computer’s connection, then try again.' }
      const url = await phoneAnywhere.deskCode()
      if (url === undefined) return { ...(await phoneAnywhere.settings()), message: 'Locust’s relay did not open a code for this browser. Try again in a moment.' }
      await openForPerson(url)
      return phoneAnywhere.settings()
    })
    ipcMain.handle(PHONE_GITHUB_IN_CHANNEL, async (event) => {
      if (!fromOwnWindow(event)) return NO_PHONE
      const url = await phoneAnywhere.githubLink()
      if (url !== undefined) await openForPerson(url)
      return phoneAnywhere.settings()
    })
    ipcMain.handle(PHONE_GITHUB_OUT_CHANNEL, (event) => fromOwnWindow(event) ? phoneAnywhere.githubSignOut() : NO_PHONE)
    /*
     * LOCUST IN A BROWSER (0.830, desk-host.ts): a paired device Settings lets open the full window makes the window's
     * calls through the relay link; this computer's window shows a cover while it does.
     */
    const coverWindow = async (on: boolean): Promise<void> => {
      const window = approvalWindow
      if (window === undefined || window.isDestroyed()) return
      const devUrl = !app.isPackaged ? process.env.ELECTRON_RENDERER_URL : undefined
      if (devUrl !== undefined) await window.loadURL(on ? `${devUrl}?${DESK_COVER_QUERY}=1` : devUrl)
      else await window.loadFile(join(__dirname, '../renderer/index.html'), on ? { query: { [DESK_COVER_QUERY]: '1' } } : {})
    }
    const deskDeviceName = async (device: string | undefined): Promise<string | undefined> =>
      device === undefined ? undefined : (await phoneAnywhere.settings()).devices.find((one) => one.id === device)?.name
    const desk: DeskHost = createDeskHost({
      handlers: deskHandlers,
      window: () => approvalWindow,
      tell: (device, items) => phoneAnywhere.tellDesk(device, items),
      cover: coverWindow,
      hello: async () => ({ version: app.getVersion(), computer: hostname(), theme: appearanceNow().resolved }),
      waiting: () => raisedApprovals.waiting().map((one) => one.request)
    })
    deskHost = desk
    deskLink = desk
    ipcMain.handle(DESK_WHERE_CHANNEL, async (event) => (fromOwnWindow(event) ? { ...((await deskDeviceName(desk.active())) === undefined ? {} : { device: await deskDeviceName(desk.active()) }) } : {}))
    ipcMain.handle(DESK_TAKE_BACK_CHANNEL, async (event) => { if (fromOwnWindow(event)) await desk.takeBack() })
    ipcMain.handle(PHONE_DESK_CHANNEL, async (event, device: unknown, on: unknown) => (fromOwnWindow(event) ? phoneAnywhere.setDesk(device, on) : NO_PHONE))
    ipcMain.handle(PHONE_NIGHT_CHANNEL, async (event, on: unknown) => {
      if (!fromOwnWindow(event)) return NO_PHONE
      const settings = await phoneAnywhere.setNightApprovals(on)
      phoneNightApprovals = settings.nightApprovals === true
      return settings
    })
    ipcMain.handle(LOCUST_MCP_GET, event => fromOwnWindow(event) ? mcpHost.settings() : { enabled: false, ownMode: false })
    ipcMain.handle(LOCUST_MCP_SET, (event, enabled: unknown) => fromOwnWindow(event) ? mcpHost.setEnabled(enabled) : { enabled: false, ownMode: false })
    ipcMain.handle(LOCUST_MCP_SET_OWN_MODE, (event, ownMode: unknown) => fromOwnWindow(event) ? mcpHost.setOwnMode(ownMode) : { enabled: false, ownMode: false })
    ipcMain.handle(LOCUST_MCP_SET_CONVERSATIONS, (event, conversations: unknown) => fromOwnWindow(event) ? mcpHost.setConversations(conversations) : { enabled: false, ownMode: false })
    ipcMain.handle(LOCUST_MCP_SET_EDITOR_ANSWERS, (event, editorAnswers: unknown) => fromOwnWindow(event) ? mcpHost.setEditorAnswers(editorAnswers) : { enabled: false, ownMode: false })
    // Locust in the Claude app's tools, in one press (0.740): Add turns the local server on and writes the one entry.
    const mcpBridge = app.isPackaged ? join(process.resourcesPath, 'locust-mcp-bridge.mjs') : join(__dirname, '../../resources/locust-mcp-bridge.mjs')
    const claudeApp = createClaudeAppConnector({
      file: claudeAppConfigFile(process.platform, process.env.APPDATA, homedir()),
      entry: () => ({ command: process.execPath, args: [mcpBridge, join(app.getPath('userData'), MCP_CONNECTION_FILE)], env: { ELECTRON_RUN_AS_NODE: '1' } }),
      startedAt: () => claudeAppStartedAt(process.platform),
      relaunch: () => relaunchClaudeApp(process.platform),
      // When Locust last changed Claude's settings and last restarted Claude, kept across a Locust restart.
      marks: {
        read: async () => {
          const saved = JSON.parse(await readFile(join(app.getPath('userData'), 'claude-app.json'), 'utf8').catch(() => '{}')) as { changedAt?: unknown; restartedAt?: unknown }
          return { ...(typeof saved.changedAt === 'number' ? { changedAt: saved.changedAt } : {}), ...(typeof saved.restartedAt === 'number' ? { restartedAt: saved.restartedAt } : {}) }
        },
        write: (marks) => writeFile(join(app.getPath('userData'), 'claude-app.json'), JSON.stringify(marks), 'utf8')
      }
    })
    ipcMain.handle(LOCUST_MCP_CLAUDE_APP_RESTART, async (event) => {
      if (!fromOwnWindow(event)) return { connection: await claudeApp.status(), message: 'Claude was not restarted.' }
      const done = await claudeApp.restart()
      return { connection: await claudeApp.status(), ...(done.ok ? {} : { message: done.message }) }
    })
    ipcMain.handle(LOCUST_MCP_CLAUDE_APP, (event) => fromOwnWindow(event) ? claudeApp.status() : { kind: 'no-app' })
    ipcMain.handle(LOCUST_MCP_CLAUDE_APP_SET, async (event, added: unknown) => {
      if (!fromOwnWindow(event) || typeof added !== 'boolean') return { connection: await claudeApp.status(), message: 'That setting was not changed.' }
      if (!added) {
        const removed = await claudeApp.remove()
        return { connection: await claudeApp.status(), ...(removed.ok ? {} : { message: removed.message }) }
      }
      const state = await mcpHost.setEnabled(true)
      if (!state.enabled) return { connection: await claudeApp.status(), state, message: state.message ?? 'The local server could not be turned on, so the Claude app was not changed.' }
      const done = await claudeApp.add()
      return { connection: await claudeApp.status(), state, ...(done.ok ? {} : { message: done.message }) }
    })

    ipcMain.handle(CODEX_MISSION_CANCEL_CHANNEL, (event, request: unknown) => {
      const owner = BrowserWindow.fromWebContents(event.sender)
      if (!owner || !event.senderFrame || event.senderFrame.parent !== null) {
        return {
          ok: false,
          error: {
            code: 'INTERNAL_ERROR',
            message: 'The cancellation request was rejected.'
          }
        } as const
      }
      const runId = typeof request === 'object' && request !== null
        ? (request as Partial<CodexMissionCancelRequest>).runId
        : undefined
      const viaExec = codexMissions.cancel(runId)
      if (viaExec.ok || typeof runId !== 'string') return viaExec
      // Not one of the mission service's runs: Antigravity owns its own, and a
      // stop control that only knew one transport reported "no longer active"
      // at a run that was very much still going.
      if (antigravityMissions.cancel(runId)) {
        return { ok: true, data: { runId, state: 'cancellation-requested' } } as const
      }
      return viaExec
    })

    ipcMain.handle(MISSION_RESUME_CHANNEL, async (event, request: unknown) => {
      const owner = BrowserWindow.fromWebContents(event.sender)
      if (!owner || !event.senderFrame || event.senderFrame.parent !== null) {
        return {
          ok: false,
          error: { code: 'INTERNAL_ERROR', message: 'The resume request was rejected.' }
        } as const
      }
      const refusedResume = noWorkspaceRefusal()
      if (refusedResume !== undefined) return refusedResume
      const payload = (typeof request === 'object' && request !== null ? request : {}) as Partial<MissionResumeRequest>
      const account = typeof payload.account === 'string' ? payload.account : undefined
      // Same widening as a start and a handoff: an unrecognized mode is
      // read-only and an unrecognized runtime is Codex, so a malformed
      // request cannot buy itself write access by being wrong.
      // M26: the same reading as a start -- Auto, Approve-each and Plan are not Ask.
      const mode = parsedMissionMode(payload.mode)
      const runtime = isMissionRuntime(payload.runtime) ? payload.runtime : 'codex'
      const model = typeof payload.model === 'string' ? payload.model : undefined
      const effort = typeof payload.effort === 'string' ? payload.effort : undefined
      try {
        // H6: resumed as the teammate the interrupted mission belonged to --
        // in their own branch or folder, briefed as them, under their guard --
        // and the continuation is recorded as theirs, as a handoff's is.
        const owners = await teammates.missionOwners().catch(() => ({}) as Readonly<Record<string, string>>)
        const ownerId = typeof payload.missionId === 'string' ? owners[payload.missionId] : undefined
        const peer = ownerId === undefined ? undefined : await peerContextFor(ownerId).catch(() => undefined)
        const response = await codexMissions.resume(
          payload.missionId,
          runtime,
          mode,
          { ...(model === undefined ? {} : { model }), ...(effort === undefined ? {} : { effort }), ...(account === undefined ? {} : { account }) },
          (update: CodexMissionUpdate) => {
            if (!owner.isDestroyed() && !owner.webContents.isDestroyed()) {
              owner.webContents.send(CODEX_MISSION_UPDATE_CHANNEL, update)
            }
          },
          peer
        )
        if (response.ok && ownerId !== undefined) await assignOwner(ownerId, response.data.missionId)
        return response
      } catch {
        return {
          ok: false,
          error: { code: 'INTERNAL_ERROR', message: 'The mission could not be resumed.' }
        } as const
      }
    })

    ipcMain.handle(MISSION_HANDOFF_CHANNEL, async (event, request: unknown) => {
      const owner = BrowserWindow.fromWebContents(event.sender)
      if (!owner || !event.senderFrame || event.senderFrame.parent !== null) {
        return {
          ok: false,
          error: { code: 'INTERNAL_ERROR', message: 'The handoff request was rejected.' }
        } as const
      }
      const refusedHandoff = noWorkspaceRefusal()
      if (refusedHandoff !== undefined) return refusedHandoff
      const payload = (typeof request === 'object' && request !== null ? request : {}) as Partial<MissionHandoffRequest>
      const account = typeof payload.account === 'string' ? payload.account : undefined
      // Same widening rules as a start: an unrecognized mode is read-only and
      // an unrecognized runtime is Codex. A handoff must not become the way a
      // malformed request buys itself write access.
      // M26: the same reading as a start -- Auto, Approve-each and Plan are not Ask.
      const mode = parsedMissionMode(payload.mode)
      const runtime = isMissionRuntime(payload.runtime) ? payload.runtime : 'codex'
      const model = typeof payload.model === 'string' ? payload.model : undefined
      const effort = typeof payload.effort === 'string' ? payload.effort : undefined
      try {
        const response = await codexMissions.handOff(
          payload.runId,
          runtime,
          mode,
          { ...(model === undefined ? {} : { model }), ...(effort === undefined ? {} : { effort }), ...(account === undefined ? {} : { account }) },
          (update: CodexMissionUpdate) => {
            if (!owner.isDestroyed() && !owner.webContents.isDestroyed()) {
              owner.webContents.send(CODEX_MISSION_UPDATE_CHANNEL, update)
            }
          }
        )
        if (response.ok) {
          // The continuation belongs to whoever the stopped mission did.
          const owners = await teammates.missionOwners().catch(() => ({}) as Readonly<Record<string, string>>)
          const ownerId = owners[response.data.continuesFrom.missionId]
          await assignOwner(ownerId, response.data.missionId)
          // A person picked this route for this teammate, mid-run; that is
          // as much a choice as starting them on it. The sidebar read the old
          // route while the composer read the new one (seen driving, 2026-09-05).
          await rememberRoute(ownerId, { runtime, model: model ?? 'account-default', mode, ...(effort === undefined ? {} : { effort }), ...(account === undefined ? {} : { account }) })
        }
        return response
      } catch {
        return {
          ok: false,
          error: { code: 'INTERNAL_ERROR', message: 'The mission could not be handed off.' }
        } as const
      }
    })

    const windowFromValidSender = (event: Electron.IpcMainEvent): BrowserWindow | undefined => {
      const window = BrowserWindow.fromWebContents(event.sender)
      if (!window || window.webContents !== event.sender || !event.senderFrame || event.senderFrame.parent !== null) return undefined
      return window
    }
    ipcMain.on('window:minimize', (event) => {
      windowFromValidSender(event)?.minimize()
    })
    ipcMain.on('window:toggle-maximize', (event) => {
      const window = windowFromValidSender(event)
      if (!window) return
      window.isMaximized() ? window.unmaximize() : window.maximize()
    })
    ipcMain.on('window:close', (event) => {
      windowFromValidSender(event)?.close()
    })
    /*
     * 0.379: the taskbar says when something needs you (taskbar-attention.ts)
     * -- the amber dot while anything does, a flash when more does while the
     * window is elsewhere. The renderer counts; only a small whole number is
     * believed.
     */
    /*
     * SINCE YOU WERE AWAY (0.590, PRD R17). The attention mark is the moment
     * the person was last at the window (main/away.ts): read here, before
     * anything writes it, so this start knows when the last one was left.
     * Written when the window loses focus, closes, or Locust quits, and once a
     * minute while it has focus. The renderer asks for the moment, reduces
     * what it already holds (shared/away.ts), and hands back the counts the
     * tray says. A window back after twenty minutes starts a new absence.
     */
    const awayFile = join(app.getPath('userData'), AWAY_FILE)
    let awaySince: string | undefined = wasAway(readAttentionMark(awayFile), new Date())
    let awayCountsShown: AwaySummaryCounts | null = null
    const markAttention = (): void => {
      try {
        writeAttentionMark(awayFile, new Date().toISOString())
      } catch (error) {
        note('away', `could not write the attention mark: ${error instanceof Error ? error.message : String(error)}`)
      }
    }
    let lastBlurAt: string | undefined
    const attendWindow = (window: BrowserWindow): void => {
      window.on('blur', () => {
        lastBlurAt = new Date().toISOString()
        markAttention()
      })
      window.on('focus', () => {
        const since = wasAway(lastBlurAt, new Date())
        lastBlurAt = undefined
        if (since === undefined) return
        awaySince = since
        if (!window.isDestroyed()) window.webContents.send(AWAY_SINCE_CHANNEL, { since })
      })
      window.on('close', () => markAttention())
    }
    const attentionBeat = setInterval(() => {
      if (BrowserWindow.getAllWindows().some((one) => !one.isDestroyed() && one.isFocused())) markAttention()
    }, 60_000)
    app.once('before-quit', () => {
      clearInterval(attentionBeat)
      markAttention()
    })
    ipcMain.handle(AWAY_SINCE_GET_CHANNEL, (event) => {
      if (!fromOwnWindow(event) || awaySince === undefined) return {}
      return { since: awaySince }
    })
    ipcMain.on(AWAY_SUMMARY_CHANNEL, (event, raw: unknown) => {
      if (!windowFromValidSender(event)) return
      awayCountsShown = awayCountsFrom(raw)
    })
    ipcMain.on(AWAY_SEEN_CHANNEL, (event) => {
      if (!windowFromValidSender(event)) return
      awaySince = undefined
      awayCountsShown = null
      markAttention()
    })
    let needsYouShown = 0
    const attentionDotImage = nativeImage.createFromBitmap(attentionDot(16), { width: 16, height: 16 })
    ipcMain.on(NEEDS_YOU_COUNT_CHANNEL, (event, raw: unknown) => {
      const window = windowFromValidSender(event)
      const count = needsYouCountFrom(raw)
      if (!window || count === undefined) return
      const said = taskbarAttention(needsYouShown, count, window.isFocused() && !window.isMinimized())
      needsYouShown = count
      window.setOverlayIcon(said.overlay === 'dot' ? attentionDotImage : null, said.overlay === 'dot' ? said.description : '')
      if (said.flash) {
        window.flashFrame(true)
        window.once('focus', () => window.flashFrame(false))
      }
    })
    // 0.379: a long run the person started finished (the renderer decides
    // which: finishedToast.ts). Said once, gathered, only while the window is
    // elsewhere; a click opens the conversation.
    ipcMain.on(RUN_FINISHED_CHANNEL, (event, raw: unknown) => {
      if (!windowFromValidSender(event)) return
      const finish = finishFrom(raw)
      if (finish !== undefined) attention.runFinished(finish)
    })

    /*
     * The loading window first, then the app behind it.
     *
     * `tube: 'off'` is a person saying they do not want the ceremony, so
     * they get no splash and the app opens straight away.
     */
    const openSplash = (tube: string): void => {
      if (tube === 'off') return
      splashWindow = createSplashWindow()
      splashWindow.on('closed', () => {
        splashWindow = undefined
        // Whatever became of the splash -- finished, or closed by hand --
        // the app must not be left invisible behind it.
        showAppWindow()
      })
    }
    // --background is the tray and no window: a splash would be a window.
    const startInBackground = process.argv.includes(BACKGROUND_ARG)
    // Read, not assumed: somebody who turned the ceremony off gets no
    // loading window and the app opens straight away. Synchronously
    // optimistic, because the splash must exist BEFORE the app window is
    // ready to show or the app would show itself first.
    if (!startInBackground) openSplash('full')
    /*
     * A CAP ON THE LOADING SCREEN.
     *
     * A probe that never answers must not mean an app that never opens --
     * that is far worse than a slow start, and it is the failure a loading
     * screen makes possible in the first place. After this the app is shown
     * and the splash closed regardless, and the runtimes carry on answering
     * behind it exactly as they did before there was a splash at all.
     */
    if (!startInBackground) {
      setTimeout(() => {
        const splash = splashWindow
        if (splash === undefined || splash.isDestroyed()) return
        splashWindow = undefined
        showAppWindow()
        splash.close()
      }, SPLASH_CAP_MS)
      void teammates
        .readSettings()
        .then((held) => {
          if (held.tube !== 'off') return
          const splash = splashWindow
          splashWindow = undefined
          if (splash !== undefined && !splash.isDestroyed()) splash.close()
        })
        .catch(() => undefined)
    }

    /*
     * 0.382: closing the window while a teammate works asks first
     * (quit-guard.ts). Only the person's own close: a quit already under
     * way (a relaunch, the updater) and a session ending pass straight
     * through, and "Quit anyway" closes.
     */
    /*
     * 0.397: KEEP WORKING IN THE BACKGROUND. The first answer closes the
     * window and keeps Locust in the tray while the runs go on -- the part
     * of Orca's "runs that outlive the window" that needs no second process:
     * the runs, the ledger, routines and the finish notification all live in
     * this one, which simply stays up. The tray says who is working; a click
     * opens the window again; its menu can quit.
     */
    let tray: Tray | undefined
    // The tray menu's labels as last built (0.587): rebuilt only when they change.
    let trayLabels: string | undefined
    let trayBeat: ReturnType<typeof setInterval> | undefined
    let trayOpen: () => void = () => undefined
    let trayQuit: () => void = () => undefined
    const workingNames = async (liveIds: readonly string[]): Promise<readonly string[]> => {
      const [roster, owners] = await Promise.all([teammates.list(), teammates.missionOwners()]).catch(() => [[], {}] as const)
      return liveIds
        .map((missionId) => roster.find((entry) => entry.teammateId === (owners as Readonly<Record<string, string>>)[missionId])?.name)
        .filter((name): name is string => name !== undefined)
    }
    const leaveTray = (): void => {
      if (trayBeat !== undefined) clearInterval(trayBeat)
      trayBeat = undefined
      tray?.destroy()
      tray = undefined
      // The next tray starts without a menu; its first beat must build one.
      trayLabels = undefined
    }
    const trayIcon = () =>
      // macOS draws a menu-bar icon from a small PNG and cannot read an .ico
      // at all: handed one, the icon is empty and a closed Locust has no
      // way back but the Dock (the first macOS build, 2026-09-29).
      // Linux's tray takes the same small PNG, at the size its panel draws it (0.789).
      process.platform === 'darwin'
        ? nativeImage.createFromPath(app.isPackaged ? join(process.resourcesPath, 'icon-32.png') : join(__dirname, '../../resources/icon-32.png')).resize({ width: 18, height: 18 })
        : process.platform === 'linux'
          ? nativeImage.createFromPath(app.isPackaged ? join(process.resourcesPath, 'icon-32.png') : join(__dirname, '../../resources/icon-32.png'))
          : nativeImage.createFromPath(app.isPackaged ? join(process.resourcesPath, 'icon.ico') : join(__dirname, '../../resources/icon-512.png'))
    /**
     * One tray, for --background and for a window that closed into it.
     * Open Locust, the next routine due, Quit.
     */
    const ensureTray = (onOpen: () => void, onQuit: () => void): void => {
      trayOpen = onOpen
      trayQuit = onQuit
      if (tray === undefined) {
        tray = new Tray(trayIcon())
        tray.on('click', () => trayOpen())
      }
      const say = (): void => {
        const liveIds = [...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()]
        void workingNames(liveIds).then((names) => tray?.setToolTip(trayLine(names, liveIds.length)))
        void routines.list().then((all) => {
          if (tray === undefined) return
          const [openLabel, dueLabel, quitLabel] = trayMenuLabels(nextRoutineDueLine(all, new Date()))
          // Since you were away (0.590): the counts the renderer handed back, as one line that opens the window.
          const awayLabel = awayLine(awayCountsShown)
          // Rebuilt only when a label changed (0.587): the beat is every five
          // seconds, and a menu replaced under an open pointer closes it.
          const labels = `${openLabel}\n${dueLabel}\n${awayLabel ?? ''}\n${quitLabel}`
          if (labels === trayLabels) return
          trayLabels = labels
          tray.setContextMenu(Menu.buildFromTemplate([
            { label: openLabel, click: () => trayOpen() },
            ...(awayLabel === undefined ? [] : [{ label: awayLabel, click: () => trayOpen() }]),
            { label: dueLabel, enabled: false },
            { label: quitLabel, click: () => { leaveTray(); trayQuit() } }
          ]))
        }).catch(() => undefined)
      }
      say()
      if (trayBeat !== undefined) clearInterval(trayBeat)
      trayBeat = setInterval(say, 5_000)
    }
    const toTray = (window: BrowserWindow, quit: () => void): void => {
      const reopen = (): void => {
        leaveTray()
        if (window.isDestroyed()) return
        window.show()
        window.focus()
      }
      ensureTray(reopen, quit)
      window.hide()
    }
    const guardClose = (window: BrowserWindow): void => {
      let confirmed = false
      let asking = false
      let sessionEnding = false
      window.on('session-end', () => {
        sessionEnding = true
      })
      // Shown again from anywhere (a notification, the taskbar): the tray has done its job.
      window.on('show', () => leaveTray())
      const quit = (): void => {
        confirmed = true
        app.quit()
      }
      window.on('close', (event) => {
        const liveIds = [...codexMissions.liveMissionIds(), ...antigravityMissions.liveMissionIds()]
        if (hideToTrayOnClose({ liveRuns: liveIds.length, appQuitting, sessionEnding, confirmed, keepRunning: readKeepRunning(backgroundFile()) })) {
          event.preventDefault()
          toTray(window, quit)
          return
        }
        if (!shouldAskBeforeClosing({ liveRuns: liveIds.length, appQuitting, sessionEnding, confirmed })) {
          leaveTray()
          return
        }
        event.preventDefault()
        if (asking) return
        asking = true
        void (async () => {
          const question = closeQuestion(await workingNames(liveIds), liveIds.length)
          /*
           * A DRIVE'S ANSWER, labelled as such: a native dialog cannot be
           * pressed through the page a drive talks to, so a drive names the
           * button it would press. `LOCUST_CLOSE_ANSWER` is read nowhere else.
           */
          const seam = ({ background: 0, quit: 1, cancel: 2 } as Record<string, number>)[process.env.LOCUST_CLOSE_ANSWER ?? '']
          const response = seam ?? (await dialog.showMessageBox(window, {
            type: 'question',
            buttons: [...CLOSE_BUTTONS],
            defaultId: 0,
            cancelId: 2,
            noLink: true,
            title: 'Locust',
            message: question.message,
            detail: question.detail
          })).response
          asking = false
          if (window.isDestroyed()) return
          if (response === 0) toTray(window, quit)
          if (response === 1) {
            confirmed = true
            window.close()
          }
        })()
      })
    }

    const openMain = (beginSweep: boolean): void => {
      createWindow(codexMissions, (window) => {
        approvalWindow = window
        mirrorToDesk(window)
        browserHost.attachWindow(window)
        guardClose(window)
        attendWindow(window)
        window.webContents.once('did-finish-load', () => setTimeout(() => void backgroundForShutdown?.resume().catch(() => undefined), 1_500))
        replayDiscoveryToWindow()
        if (!beginSweep) return
        // The sweep may begin: there is somebody to watch it now.
        windowIsUp()
        /*
         * AND IT DOES BEGIN, rather than waiting to be asked.
         *
         * The first sweep used to start when the app's renderer asked for it,
         * which is after its whole bundle has loaded and run: measured 0.84 s
         * into a launch, with the window itself there at 0.37 s (2026-09-22).
         * The renderer's ask now joins the sweep already under way.
         */
        void discoverForWork().catch(() => undefined)
      })
    }
    const openExistingOrMain = (): void => {
      const existing = BrowserWindow.getAllWindows().find((one) => !one.isDestroyed())
      if (existing !== undefined) {
        if (existing.isMinimized()) existing.restore()
        existing.show()
        existing.focus()
        return
      }
      openMain(true)
    }
    openFromBackground = openExistingOrMain
    if (startInBackground) {
      ensureTray(openExistingOrMain, () => { app.quit() })
      console.error('Locust is in the tray.')
    } else {
      openMain(true)
    }

    app.on('activate', () => {
      // The Dock icon brings back a window kept working in the background
      // (macOS): it is hidden, not gone, so "no windows" was never true of it.
      const hidden = BrowserWindow.getAllWindows().find((one) => !one.isDestroyed() && !one.isVisible())
      if (hidden !== undefined) {
        hidden.show()
        hidden.focus()
        return
      }
      if (BrowserWindow.getAllWindows().length === 0) {
        createWindow(codexMissions, (window) => {
          approvalWindow = window
          mirrorToDesk(window)
          browserHost.attachWindow(window)
          guardClose(window)
          attendWindow(window)
          replayDiscoveryToWindow()
        })
      }
    })
  }).catch((error: unknown) => {
    console.error('Failed to initialize Teammate', error)
    app.exit(1)
  })
}

/** Set the moment any quit begins, so the window's close guard never stands in its way (quit-guard.ts). */
let appQuitting = false
app.on('before-quit', () => {
  appQuitting = true
  // Locust stops WATCHING its background turns; Claude Code keeps running them (W10).
  backgroundForShutdown?.dispose()
})

app.on('window-all-closed', () => {
  void voiceForShutdown?.cancel()
  if (process.platform === 'darwin') return
  // A quit from the tray is already leaving. Keeping the process up is only
  // for a window that closed while the person asked Locust to stay.
  if (appQuitting) {
    app.quit()
    return
  }
  if (readKeepRunning(join(app.getPath('userData'), BACKGROUND_FILE))) return
  app.quit()
})

/**
 * What to do instead of the final `app.quit()`, when something asked for the
 * quit in order to do it. Today that is the updater: an installer can only be
 * launched once the ledger is safely on disk.
 */
let quitFinaliser: (() => void) | undefined
let shutdownStarted = false
let shutdownComplete = false
/** How long the flush below is allowed to take -- see `boundedShutdown`. */
const SHUTDOWN_DEADLINE_MS = 6_000
if (ownsSingleInstanceLock) {
  app.on('before-quit', (event) => {
    if (shutdownComplete) return
    event.preventDefault()
    if (shutdownStarted) return
    shutdownStarted = true
    boundedShutdown({
      deadlineMs: SHUTDOWN_DEADLINE_MS,
      work: async () => {
        await terminalForShutdown?.dispose()
        ownCommandsForShutdown?.stopAll()
        await browserForShutdown?.dispose()
        await mcpForShutdown?.dispose()
        await voiceForShutdown?.dispose()
        await remoteControlForShutdown?.dispose()
        await queuedMessagesForShutdown?.flush()
        await missionServiceForShutdown?.dispose()
        await antigravityServiceForShutdown?.dispose()
        await ledgerForShutdown?.flush()
        await workroomForShutdown?.flush()
        // Refuses whatever a run was still asking, and closes the loopback door.
        await permissionHostForShutdown?.dispose()
      },
      leave: (reason) => {
        shutdownComplete = true
        // Whoever asked for the quit gets the last word: the updater installs
        // and starts the app again. Anything else is an ordinary quit.
        const finalise = quitFinaliser
        quitFinaliser = undefined
        if (finalise !== undefined) {
          finalise()
          return
        }
        if (reason === 'finished') {
          app.quit()
          return
        }
        // The wait was abandoned, so re-entering the quit would only wait
        // again. `app.exit` is the one way out that a stuck child process
        // cannot hold, and the relaunch helper watches the process rather
        // than its exit code, so a folder switch still reopens from here.
        console.error(`Shutdown did not finish within ${String(SHUTDOWN_DEADLINE_MS)}ms; leaving anyway.`)
        app.exit(0)
      },
      failed: (error) => {
        console.error('Failed to flush the local mission ledger during shutdown', error)
        app.exit(1)
      }
    })
  })
}

/** Names already in the attachments folder, so a copy never overwrites one. */
async function readAttachmentNames(workspacePath: string): Promise<readonly string[]> {
  try {
    return await readdir(join(workspacePath, ATTACHMENT_DIR))
  } catch {
    // No folder yet is the ordinary first case, not a failure.
    return []
  }
}

/**
 * Keep the attachments folder out of the person's commits.
 *
 * `.git/info/exclude`, not `.gitignore`: their ignore file is tracked and
 * belongs to them, and a line Locust added would show up as a change they did
 * not make. A workspace that is not a git repository has neither, and nothing
 * here needs to happen.
 */
async function keepAttachmentsOutOfGit(workspacePath: string): Promise<void> {
  const excludePath = join(workspacePath, '.git', 'info', 'exclude')
  try {
    let current = ''
    try {
      current = await readFile(excludePath, 'utf8')
    } catch {
      // A repository with no exclude file yet: write one, but only if the
      // folder it belongs in is really there, so this cannot invent a .git.
      await readdir(join(workspacePath, '.git'))
      await mkdir(join(workspacePath, '.git', 'info'), { recursive: true })
    }
    const next = excludeWith(current)
    if (next !== undefined) await writeFile(excludePath, next, 'utf8')
  } catch {
    // Not a repository, or an unwritable one. The attachment still works;
    // this was only ever tidiness.
  }
}
