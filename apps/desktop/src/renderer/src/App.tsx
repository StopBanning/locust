import { accountUsageKey, accountFromRoute } from '../../shared/agent-accounts.js'
import { accountName, routeAccount, useAgentAccounts } from './agentAccounts.js'
import { lazy, Suspense, useEffect, useMemo, useRef, useState } from 'react'
import { TerminalToggle } from './components/TerminalToggle.js'
import { BrowserToggle } from './components/BrowserToggle.js'
import { isTerminalToggle } from './terminalKeys.js'
import { policyBrowserRefusal, policyTerminalRefusal } from '../../shared/organization-policy.js'
const TerminalPanel = lazy(() => import('./components/TerminalPanel.js'))
const BrowserPanel = lazy(() => import('./components/BrowserPanel.js'))
import { BOT_SIZE } from './botSizes.js'
import { OrganizationPolicyContext, OrganizationPolicyNotice } from './organizationPolicy.js'
import { POLICY_MODE_ORDER, policyModeAllowed } from '../../shared/organization-policy.js'
import { commandsStillToCarry, promptWithCommands } from '../../shared/own-commands.js'
import type { ListedIssue, StartedIssue } from '../../shared/github-issue.js'
import type { ConversationCommand, ListedCommand, OwnCommandResult } from '../../shared/own-commands.js'
import { CommandEntry } from './components/OwnCommand.js'
import type { ReactElement } from 'react'

import type { MissionRuntimeId, NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import type {
  PublicFolder,
  KeepRunningState,
  LoginItemState,
  RuntimeUpdatesState,
  LayoutPreference,
  TubePreference,
  ReplyTextSize,
  MetalPreset,
  MetalStrength,
  MetalMotion,
  CodexMissionUpdate,
  MissionRouteSummary,
  GroupMembership,
  LeftMembership,
  PublicGroup,
  PublicRecoveredMission,
  PublicRuntimeStatus,
  PublicStorageReport,
  MissionPruneResponse,
  ProfileBackupResponse,
  ProfileRestoreOutcome,
  ProfileRestorePreview,
  ProfileRestoreResponse,
  AppChangelog,
  TrashListResponse,
  TrashMutationResponse,
  AppUpdateResponse,
  AppUpdateState,
  MissionApprovalDecision,
  MissionApprovalRequest,
  MissionApprovalAnswer,
  MissionHistoryResponse,
  MissionMode,
  PublicModel,
  PublicRuntimeArtifact,
  PublicPeerMessage,
  PublicRoutine,
  PublicTeammate,
  TeammateRoute, PublicRoom, RoomTaskRequest,
  RoutineHandOff,
  RoutineSchedule,
  MemoryListResponse,
  MemoryMode,
  MemoryScope,
  MemoryUpdateRequest,
  PublicForgottenMemory,
  PublicMemory,
  PublicRuntimeSetup,
  PublicWorkspaceBrief,
  PublicWorktree,
  PublicConnector
} from '../../shared/ipc.js'
import { roleLabelOf } from '../../shared/ipc.js'
import { addPagePoint, MAX_PAGE_POINTS, noteOnPagePoint, withoutPagePoint, type PagePointChip } from '../../shared/browser-point.js'
import { isPanelRunKind } from '../../shared/panel-runs.js'
import { carryOnOffer, planRanOut } from '../../shared/carry-on.js'
import type { PlanRunsOut } from '../../shared/carry-on.js'
import { withSuggestion } from '../../shared/about-you.js'
import { aboutYouSecretRefusal, secretIn } from '../../shared/secrets.js'
import type { AboutYouSuggestion } from '../../shared/about-you.js'
import type { Workbook } from '../../shared/sheet.js'
import type { OfficeDocument } from '../../shared/office-document.js'
import type { Spend } from '../../shared/spend.js'
import { chainDraftFrom, routineDraft, routineStepPhrase } from './routines.js'
import { queueHome, combineQueued, queuedIn, queuedVerdict, requeuedRows, retriedAfterBusy, withoutQueueOf } from './steering.js'
import type { QueuedRow } from './steering.js'
import { useConversationQueue } from './useConversationQueue.js'
import type { RoutineDraft } from './routines.js'
import { RoutineDialog } from './components/RoutineDialog.js'
import { RoutineRunDialog } from './components/RoutineInputs.js'
import { RoutineExportDialog, RoutineImportDialog } from './components/RoutineFileDialog.js'
import type { RoutineInput, RoutineValues } from '../../shared/routine-inputs.js'
import type { RoutineImportPreview, RoutineFlaggedPath, RoutineTemplateInfo } from '../../shared/ipc.js'
import { RoutineTemplateDialog } from './components/RoutineTemplates.js'
import { AutomationsScreen } from './components/AutomationsScreen.js'
import { BenchmarksScreen, type BenchmarkEntry } from './components/BenchmarksScreen.js'
import { TIDY_PROMPT } from '../../shared/memory-tidy.js'
import { needsYou, needsYouLabel, waitingByTeammate } from './needsYou.js'
import { withNote } from './diffNotes.js'
import type { DiffNote } from './diffNotes.js'
import { DiffNotesContext } from './components/DiffNotes.js'
import type { DiffNotesPlace } from './components/DiffNotes.js'
import { memoryChangedNotice, memoriesOfConversation, noticeWaits, turnsOfConversation } from './conversationMemories.js'
import { createFrameBatcher } from './streamFrames.js'
import { missingTranscripts, heldDigests, mergeHistory } from './historyMerge.js'
import { recoverLiveRuns } from './recoverLiveRuns.js'
import { savableMissionId } from './savableConversations.js'
import { MemoryScreen } from './components/MemoryScreen.js'
import { isMissionRuntime, runtimeDisplayName } from '../../shared/runtimes.js'
import type { StartPhase } from '../../shared/ipc.js'
import { carriedStartPhase, startingLabelOf, withStartPhase } from './startPhase.js'
import { lastWorkedPhrase, teamCardState, waitingLine } from './homeTeamState.js'
import { glassStatus } from './homeCoverStatus.js'
import { reviewPairOf } from './review-pair.js'
import type { EditCheckShown } from './components/EditCheckCard.js'
import { imageMediaType } from '../../shared/image-files.js'
import { signInCommand } from '../../shared/runtime-install.js'
import { SIGN_IN_OPENED_EVENT } from './signInEvents.js'
import { DEFAULT_RELAY_HOP_CAP, DEFAULT_MEMORY_MODE } from '../../shared/ipc.js'
import type { PublicClaudeCloudSession, PublicCloudFolder, PublicCloudTask, PublicCloudWhere, RewindPutBackResponse, RuntimeCommandsResponse } from '../../shared/ipc.js'
import { stripTaskBlocks } from '../../shared/room-task.js'
import { stripMemoryBlocks } from '../../shared/memory.js'
import { stripDecisionBlocks } from '../../shared/decision.js'
import { stripShareBlocks } from '../../shared/peer-share.js'
import { stripFileBlocks } from '../../shared/handover.js'
import { Composer } from './components/Composer.js'
import { ExchangeStrip } from './components/ExchangeStrip.js'
import { RoomScreen } from './components/RoomScreen.js'
import type { RoomAnswer } from './components/RoomScreen.js'
import { refusalNotice } from './components/RoomScreen.js'
import { exchangeAcross, exchangeOf } from './exchange.js'
import type { ExchangeMission } from './exchange.js'
import { FirstLaunch } from './components/FirstLaunch.js'
import { CommandPalette } from './components/CommandPalette.js'
import { SaveRecordDialog } from './components/SaveRecordDialog.js'
import type { PaletteAction } from './components/CommandPalette.js'
import { IdleTeammate } from './components/IdleTeammate.js'
import { Inspector } from './components/Inspector.js'
import { FileViewer } from './components/FileViewer.js'
import { MacUpdateBanner, MissionsScreen, SettingsScreen, TeammatesScreen, UpdateBanner } from './components/Screens.js'
import { BoardScreen } from './components/BoardScreen.js'
import { awayCounts, sinceYouWereAway } from '../../shared/away.js'
import { WhatsNewSplash } from './components/WhatsNew.js'
import type { SettingsPageId } from './settingsPages.js'
import type { ComparePick, ComparePicking, RouteChoice } from './components/RoutePicker.js'
import { CompareView, joinedNames } from './components/CompareView.js'
import { comparisonOf, foldComparisons } from './compareRows.js'
import type { CompareColumnView, JudgeView } from './components/CompareView.js'
import { blindName, changesLine, compareMembership, hasColumnInstructions, setupName, compareRecord, compareNeedsCopy, compareRefusalOf, judgeMissionIds, judgePickOf, MAX_COMPARE_SLOTS, MIN_COMPARE_SLOTS, rerunMissionIds, rerunRefusalOf } from '../../shared/compare.js'
import { criteriaLines, criteriaWeights, panelPick, panelVote, parseScorecard, scoreOf, weighted, withoutScorecard } from '../../shared/compare-score.js'
import type { CompareRoute, CompareSlotId, PublicCompare, RerunProgress } from '../../shared/compare.js'
import { benchmarkChart, benchmarkRows, type BenchmarkRow, type BenchmarkRunInput } from './benchmark.js'
import type { BenchmarkView } from './components/CompareBenchmark.js'
import { folderLabels } from '../../shared/folder-sections.js'
import { nestedUnder } from '../../shared/nested-conversations.js'
import { defaultComparePicks } from './compareDefaults.js'
import { effortName } from './effortLevels.js'
import { asReviewer, composerRouteFor, pickKeyFor, startAs } from '../../shared/route-at-start.js'
import type { StartAs } from '../../shared/route-at-start.js'
import type { Screen } from './components/Screens.js'
import { Icon } from './components/Icon.js'
import { NewTeammateDialog } from './components/NewTeammateDialog.js'
import type { TeammateDraft } from './components/NewTeammateDialog.js'
import { templateAvatar } from './components/TeamTemplates.js'
import { TEAM_TEMPLATES } from '../../shared/team-templates.js'
import type { TeamTemplate } from '../../shared/team-templates.js'
import { ImportDialog } from './components/ImportDialog.js'
import { UsageDialog } from './components/UsageDialog.js'
import type { UsageRange } from '../../shared/usage.js'
import { SelectionAsk } from './components/SelectionAsk.js'
import { GroupSettingsDialog } from './components/GroupSettingsDialog.js'
import { TeammateBot } from './components/TeammateBot.js'
import { BesideConversation } from './components/BesideConversation.js'
import { SideChat } from './components/SideChat.js'
import { TuneTeammate, type TuneCard } from './components/TuneTeammate.js'
import { KeptInstructions } from './components/KeptInstructions.js'
import { currentDrafts, parseTuneDrafts, tuneRefusal, tuneRoutes } from '../../shared/tune.js'
import { assistantMessages } from './missionView.js'
import { CloudTasks } from './components/CloudTasks.js'
import { CloudSetupDialog, type CloudSetupState } from './components/CloudSetupDialog.js'
import { CLOUD_AGENT_NAMES, type CloudAgent, type PublicCloudTurn } from '../../shared/cloud-computer.js'
import { BackgroundRuns } from './components/BackgroundRuns.js'
import type { PublicBackgroundRun } from '../../shared/background.js'
import { ReviewChanges } from './components/ReviewChanges.js'
import { CommitChanges } from './components/CommitChanges.js'
import { PullRequestChip } from './components/PullRequestChip.js'
import { ShareTeamDialog } from './components/TeamCard.js'
import { RuntimeMark } from './components/RuntimeMark.js'
import { terminalOffer } from './components/OpenInTerminal.js'
import type { TerminalOffer } from './components/OpenInTerminal.js'
import { Sidebar } from './components/Sidebar.js'
import type { SidebarMission } from './components/Sidebar.js'
import { ContextMenu } from './components/ContextMenu.js'
import type { ContextMenuItem, ContextMenuState } from './components/ContextMenu.js'
import { reviewMenuItem } from './reviewMenu.js'
import { Thread } from './components/Thread.js'
import { AgentAvatar, REGISTER_WORD } from './components/ThreadItems.js'
import { TitleBar } from './components/TitleBar.js'
import {
  conversationTurns,
  failureMessage,
  failedOnItsLimit,
  modelRetired,
  modelUnavailable,
  retiredFreeModels,
  downFreeModels,
  failedOnProviderSide,
  recentlyUsedRoutes,
  freeModelsThatAnswered,
  resolvedModelNames,
  resumableSessionOf,
  laterFileChanges,
  relayedTitle,
  peerRunFor,
  rootMission,
  startedLabel,
  stitchedHandoff,
  runAgainOffer, shownPrompt, typedPrompt, buildThread, durationText, runSpanMs, lastActivityAt, relativePath, fileTurns, shellCommandText, turnText, groupBoundary, groupJoins, groupLeavings, latestSetupNotes } from './missionView.js'
import type { FilePutBack, LiveStarter, ThreadItem, TurnSwitch, TurnVersions } from './missionView.js'
import { finishedToast, lastSaid } from './finishedToast.js'
import { folderName, ranOnLine } from './ranOn.js'
import { reviewBrief } from './reviewBrief.js'
import type { ReviewMaterial } from './reviewBrief.js'
import { conversationCostLine, costLine, headerCostTail, latestContext, missionCost, runCostOf, sumCosts } from './cost.js'
import { compareRecordRows } from './compareRecord.js'
import { ApprovalRuleContext } from './approvalRuleContext.js'
import type { CardRule } from './approvalRuleContext.js'
import { ruleCandidateOf, ruledActionOf, ruleSentence } from '../../shared/approval-rules.js'
import { sequenceOfPost } from './roomExchange.js'
import type { LiveTurn, RoomExchange, StartingReply } from './roomExchange.js'
import { isStoppable, stopPress } from './stopPress.js'
import { workSignature } from './useCoverActivity.js'
import { isLayoutPreference, resolveLayout } from './layout.js'
import { decisionReply } from '../../shared/decision.js'
import { installCommand } from '../../shared/runtime-install.js'
import { splitAttachments, withAttachments } from '../../shared/attachments.js'
// Only `heldFor`: this file has its own `ownerOf` for live runs, which is a
// different question from who owns a recorded mission.
import { conversationKeys, heldFor, liveRunLastAt, routineOf } from './conversationList.js'
import { stepConversation } from './conversationSteps.js'
import { snoozeChoices } from './settled.js'
import { PullRequestNewsBanner } from './components/PullRequestNewsBanner.js'
import { ChangesPanel } from './components/ChangesPanel.js'
import type { PullRequestNews } from '../../shared/pull-request.js'
import type { SettledEntry } from './settled.js'
import { collapseConversations, staleChecking, defaultEffort, defaultRoute, effortAfterRouteChange, routeAfterKeep, effortIsInModelId, modelFamily, listedAsMission, modeFacts, modeRunsOn, modesFor, modeUnavailableReason, ownerToSelect, facePresenceFor, keepWhatWasKnown, runtimeOfTeammate, runtimeIsUsable, runtimeReach, teammateStatusView, startRoute, freeStartStillFree, freeStartModel, nextFreeModel, integrationOf, ACCOUNT_DEFAULT_MODEL} from './status.js'
import { homeRouteOf, isOwnRoute, modelDisplayName, rememberOwnModels, routeChrome, routeModelName } from './routeName.js'
import { restoreNoticeLine } from './backupWords.js'
import { FeedbackDialog } from './components/FeedbackDialog.js'
import { conversationText } from './feedback.js'
import { withLiveEvents } from './liveEvents.js'
import { setPlush, setTerminalFaces } from './botLook.js'
import { DONE_HOP_MS, RECEIVED_GLANCE_MS, liveActivityOf } from './faceState.js'
import type { Handoff } from './glances.js'
import type { LiveActivity } from './faceState.js'
import type { TeammateStatusView } from './status.js'
import { shortAgo } from './railFlyout.js'
import type { WorktreeRemoval } from './components/WorktreeRow.js'
import { approvalsOfLiveRuns } from './approvalsOfLiveRuns.js'
import { addCallHandOff, addLiveCallTurn, liveCall, useCallHandOffs, useCallsSaved, useLiveCall } from './liveCall.js'
import { CallTodosContext, callTodoState, type CallTodoState, type CallTodos } from './callTodos.js'
import { pressTalk } from './talkPress.js'
import { useScoutState } from './scoutState.js'
import { ScoutCards, ScoutInvite, ScoutLooked, ScoutWatching } from './components/ScoutCards.js'
import type { ScoutDraft, ScoutOpen } from '../../shared/scout.js'
import type { TalkShortcutPress } from '../../shared/talk-shortcut.js'
import type { SavedCall } from '../../shared/calls.js'

/** Home's Not now on Meet Scout (0.756): this window's, and Settings can still turn Scout on. */
const SCOUT_NOT_NOW = 'locust.scout.not-now'

/** The board's trash set (0.585): the sidebar's rows already leave deleted conversations out. */
const NO_TRASH: ReadonlySet<string> = new Set()

/**
 * The Locust shell.
 *
 * Missions run side by side now, one per teammate. Every run the shell knows
 * about -- live, finished this session, or reopened from the ledger -- lives
 * in one map keyed by its runId, and ONE of them is on screen. Host updates
 * are addressed by runId, so a run keeps receiving them whether or not it is
 * the one being looked at; that is what makes switching threads mid-run safe.
 *
 * Carried over unchanged from the single-run shell: persist-before-emit
 * ordering, queued updates for a run whose start receipt has not arrived
 * yet, and a restored receipt that becomes live again on any update.
 */

type LiveRunPhase =
  | 'starting'
  | 'running'
  | 'cancelling'
  | 'completed'
  | 'cancelled'
  | 'failed'
  | 'interrupted'

interface LiveRunState {
  readonly prompt: string
  readonly data?: MissionRouteSummary
  readonly phase: LiveRunPhase
  readonly events: readonly NormalizedRuntimeEvent[]
  readonly eventsTruncated?: boolean
  readonly error?: string
  readonly errorIsPersistence?: boolean
  /**
   * True when this turn continues the conversation on screen but the runtime
   * had no session to resume -- the previous turn failed before one existed.
   * The exchange is still one thread; the model just starts without it, and
   * the thread says so rather than letting a person assume it remembers.
   */
  readonly coldStart?: boolean
  /** Started again from an edited earlier message (0.498): the thread says so above it. */
  readonly rewound?: boolean
  /** What putting the later replies' files back did (0.502): said in the "started again" note. */
  readonly putBack?: RewindPutBackResponse
  readonly restored?: boolean
  readonly restoredMission?: PublicRecoveredMission
  /** Who the run was messaged to, known before the host has even assigned a missionId. */
  readonly teammateId?: string
  /**
   * The runtime the message was sent to, known before the host answers. A
   * run that fails before it starts has no route summary, and the header
   * was naming Codex for an Antigravity refusal (Colin's screenshot,
   * 2026-09-05).
   */
  readonly runtime?: MissionRuntimeId
  /**
   * Earlier turns of this conversation, oldest first. Held in renderer state
   * so a reply shows the exchange immediately rather than after a history
   * refresh; recovered missions rebuild the same list from the ledger.
   */
  readonly earlierTurns?: readonly {
    readonly missionId: string
    readonly prompt: string
    readonly events: readonly NormalizedRuntimeEvent[]
    /** That turn's own workroom exchange, so the thread can draw it in place. */
    readonly peerMessages?: readonly PublicPeerMessage[]
    /** Who started that turn, where it was not the person (a turn carried on after a plan ran out, 0.774). */
    readonly startedBy?: LiveStarter
    /** Set when that turn was a reply sent to another runtime: the seam before it. */
    readonly switchedFrom?: TurnSwitch
    /** Set when the person had that exchange in the runtime's own terminal (0.391). */
    readonly inTerminal?: MissionRuntimeId
    /** The other version of that turn, when a message there was edited (0.498). */
    readonly versions?: TurnVersions
  }[]
  /** The other version of THIS turn, when it is an edit or was replaced by one (0.498). */
  readonly versions?: TurnVersions
  /**
   * Set when THIS turn is a reply sent to another runtime than the turn
   * before it: the divider drawn above the reply. From the start receipt,
   * so the seam shows the moment it happens; a reopened conversation
   * rebuilds the same one from the record (`switchOf`).
   */
  readonly switchedFrom?: TurnSwitch
  /**
   * What this run continues, when it was started by a route switch. Held in
   * renderer state rather than re-read from the ledger because the thread has
   * to show the seam the moment it happens, not after a history refresh.
   */
  readonly handoff?: {
    readonly from: MissionRuntimeId
    readonly to: MissionRuntimeId
    readonly fromModel?: string
    readonly toModel?: string
    readonly at: string | undefined
    readonly unsettledCount: number
    readonly omittedBriefing: readonly string[]
    /** What the person chose to leave out of the brief (0.527). */
    readonly leftOutByYou?: readonly string[]
    readonly priorEvents: readonly NormalizedRuntimeEvent[]
  }
  /**
   * Set when the HOST started this run -- the relay, answering for a teammate.
   * Its prompt is a briefing written to a runtime, so nothing may show it as a
   * mission title while this is set.
   */
  /** Who started it. The room kind is the window's own -- a person's post, filed under a room. */
  readonly startedBy?: LiveStarter
  /** This run was asked to PLAN rather than do, so the offer after it is the build step. */
  readonly plan?: boolean
  /**
   * When the person pressed send, ISO. The thread's waiting line clocks the
   * launch from here -- before the first event there is nothing else to time,
   * and no clock meant no line at all.
   */
  readonly startedAtIso?: string
  /** Workroom messages this run received or posted, as the host reported them. */
  readonly peerMessages?: readonly PublicPeerMessage[]
  /** Shares the host could not honour, in the host's words. */
  /** What the exchange around this run said, each at its own level (0.366). */
  readonly peerNotices?: readonly { readonly message: string; readonly level: 'info' | 'warning' }[]
  /** A3.3: the person's check after this turn changed files. */
  readonly editCheck?: EditCheckShown
  /**
   * The person pressed Stop on this run, in this window. A run can also be
   * stopped by a teammate's message (the relay's interrupt), and its card
   * must not say "You stopped this run" then (code review B4, (f)).
   */
  readonly stopPressed?: boolean
  /** The start's phase, from the main process, while the run is starting (0.602). */
  readonly startPhase?: StartPhase
}

type RuntimeDiscoveryState =
  | { readonly phase: 'loading' }
  | {
      readonly phase: 'ready'
      readonly runtimes: readonly PublicRuntimeStatus[]
      readonly npmPresent?: boolean
      /** The npm that will run is this app's own, because the machine has none. */
      readonly npmIsBundled?: boolean
      readonly npmDidNotAnswer?: boolean
      /**
       * Discovery asked three more times and something installed still never
       * answered. Grok, pass 11: five CLIs on PATH that hang forever left
       * every row CHECKING with the Install buttons gone and no way past it.
       * CHECKING is a moment; after the fourth sweep it is a verdict.
       */
      readonly gaveUp?: boolean
    }
  | { readonly phase: 'error' }

type RunMap = ReadonlyMap<string, LiveRunState>

function liveRunIsActive(run: LiveRunState | undefined): boolean {
  // The same set `stopPress` reads. Kept in one place because the button and
  // the press disagreeing is exactly how a stop came to do nothing.
  return run !== undefined && isStoppable(run.phase)
}

function isTerminal(phase: LiveRunPhase): boolean {
  return phase === 'completed' || phase === 'failed' || phase === 'cancelled'
}

function applyMissionUpdate(run: LiveRunState, update: CodexMissionUpdate): LiveRunState {
  // Any update for this runId proves the host still owns the run: a receipt
  // restored from the ledger stops being "restored" and becomes live again, so
  // the stop control and live status reflect the real process.
  let live: LiveRunState = run
  if (run.restored === true) {
    const { error: _staleError, restoredMission: _staleMission, ...rest } = run
    live = { ...rest, restored: false, phase: 'running' }
  }
  if (update.kind === 'transport-error' || update.kind === 'persistence-error') {
    return {
      ...live,
      phase: 'failed',
      error: update.error.message,
      errorIsPersistence: update.kind === 'persistence-error'
    }
  }
  if (update.kind === 'peer-message') {
    return { ...live, peerMessages: [...(live.peerMessages ?? []), update.message] }
  }
  if (update.kind === 'peer-share-failed' || update.kind === 'relay-notice') {
    return {
      ...live,
      peerNotices: [...(live.peerNotices ?? []), { message: update.message, level: update.kind === 'relay-notice' ? (update.level ?? 'warning') : 'warning' }]
    }
  }
  if (update.kind === 'edit-check') {
    const { kind: _kind, runId: _runId, missionId: _missionId, ...shown } = update
    return { ...live, editCheck: shown }
  }
  // A host-started run is adopted by the listener, never applied to a run.
  if (update.kind === 'mission-started') return live
  if (update.kind === 'start-phase') return live
  // The cloud computer's turn cards and its setting up are kept beside the runs (0.761).
  if (update.kind === 'cloud-turn' || update.kind === 'cloud-setup' || update.kind === 'cloud-sign-in') return live
  // A comparison run again (0.763) is the comparisons' business.
  if (update.kind === 'compare-reruns') return live
  // A hop being started belongs to a teammate, not to any one run.
  if (update.kind === 'relay-starting' || update.kind === 'relay-start-settled') return live
  // A room's board moving is the room's business, not this run's.
  if (update.kind === 'room-changed') return live
  if (update.kind === 'room-posted') return live
  // Memory moving is the Memory screen's business, not this run's.
  if (update.kind === 'memory-changed') return live
  // A runtime's command list is the `/` menu's business (0.426).
  if (update.kind === 'runtime-commands-changed') return live
  // So is a runtime's model list the picker's (0.697).
  if (update.kind === 'models-changed') return live
  // Background turns are the panel's business (W10).
  if (update.kind === 'background-changed') return live
  // A scheduled routine that would not start has no run to belong to.
  if (update.kind === 'routine-blocked') return live
  if (update.kind === 'routine-recovery-changed') return live

  if (update.kind === 'message-deltas') {
    return {
      ...live,
      ...withLiveEvents(live, update.events),
      phase: live.phase === 'starting' ? 'running' : live.phase
    }
  }

  /*
   * The cap is on THINGS THAT HAPPENED, and a reply is one of them.
   *
   * This was `[...live.events, update.event].slice(-500)`, and a reply
   * arrives as hundreds of fragments -- so a long answer pushed its own
   * beginning out of the window while it was still being written. Colin
   * watched it, 2026-09-11: "im currently watching it eat text from the
   * beginning", and then the other half of the symptom, which is the proof:
   * "the text seems to go back to normal after they are done". It did,
   * because a finished run is re-read from the record, where the fragments
   * are joined. Now they are joined here too, as they arrive.
   */
  // H4: the FIRST event is kept past the cap, as the history projection
  // keeps it -- it is how a run is known to have started at all.
  const window = withLiveEvents(live, [update.event])
  live = { ...live, ...window }
  const events = window.events
  if (update.event.type === 'run.completed') return { ...live, events, phase: 'completed' }
  if (update.event.type === 'run.cancelled') return { ...live, events, phase: 'cancelled' }
  if (update.event.type === 'run.failed') {
    // The payload's own sentence plus whatever the runtime actually said.
    return { ...live, events, phase: 'failed', error: failureMessage(update.event.payload, update.event.sourceAdapter) }
  }
  return { ...live, events, phase: live.phase === 'starting' ? 'running' : live.phase }
}

/**
 * Whether a turn on screen was had in its runtime's own terminal (0.391), as
 * the field every copy of a turn carries. One place, because the copy a reply
 * makes once dropped it and drew "Back in Locust" above the turn it answered.
 */
function inTerminalOf(run: LiveRunState): { readonly inTerminal?: MissionRuntimeId } {
  return run.restoredMission?.startedBy?.kind === 'terminal' ? { inTerminal: run.restoredMission.runtime } : {}
}

/**
 * A recorded turn Locust carried on after a plan ran out (0.774) keeps saying so when the conversation is opened
 * again: without its starter the thread read the host's instruction back as the person's own message (the 0.774
 * drive's reloaded frame). Only this kind: what the others carry when reopened is unchanged.
 */
function carriedOnOf(mission: PublicRecoveredMission | undefined): { readonly startedBy?: LiveStarter } {
  return mission?.startedBy?.kind === 'carry-on' ? { startedBy: mission.startedBy } : {}
}

/**
 * The turns before a host-started reply, so it renders as the next turn of
 * the conversation it continues. Read from the live run when the renderer
 * still holds it, else from history; empty when neither knows the mission,
 * in which case the thread shows this turn alone rather than nothing.
 */
function earlierTurnsOf(
  missionId: string,
  runs: RunMap,
  byId: ReadonlyMap<string, PublicRecoveredMission>
): LiveRunState['earlierTurns'] {
  const live = [...runs.values()].find((run) => run.data?.missionId === missionId)
  if (live !== undefined) {
    return [
      ...(live.earlierTurns ?? []),
      // With what that turn exchanged: the message this reply answers was
      // SENT on it. Rebuilt without it (as this was until 0.18.2) the thread
      // drew the answer and never the question, the same omission the
      // person-typed follow-up path fixed on 2026-09-04. relay-smoke run 5.
      {
        missionId,
        prompt: live.prompt,
        events: live.events,
        ...(live.peerMessages === undefined ? {} : { peerMessages: live.peerMessages }),
        ...(live.startedBy === undefined ? {} : { startedBy: live.startedBy }),
        ...(live.switchedFrom === undefined ? {} : { switchedFrom: live.switchedFrom }),
        ...(live.versions === undefined ? {} : { versions: live.versions }),
        ...inTerminalOf(live)
      }
    ]
  }
  const held = byId.get(missionId)
  if (held === undefined) return []
  return conversationTurns(held, byId).map((turn) => {
    const record = byId.get(turn.missionId)
    return {
      missionId: turn.missionId,
      prompt: record === undefined ? turn.prompt : typedPrompt(record, byId),
      events: turn.events,
      peerMessages: turn.peerMessages,
      ...carriedOnOf(record),
      ...(turn.switchedFrom === undefined ? {} : { switchedFrom: turn.switchedFrom }),
      ...(turn.inTerminal === undefined ? {} : { inTerminal: turn.inTerminal }),
      ...(turn.versions === undefined ? {} : { versions: turn.versions })
    }
  })
}

function reopenedRun(
  mission: PublicRecoveredMission,
  byId: ReadonlyMap<string, PublicRecoveredMission>
): LiveRunState {
  const restored = restoredLiveRun(mission)
  const handoff = stitchedHandoff(mission, byId)
  const turns = conversationTurns(mission, byId)
  const earlier = turns.slice(0, -1)
  const switchedFrom = turns.at(-1)?.switchedFrom
  return {
    ...restored,
    // The words a PERSON typed for this turn: a route switch's own prompt is
    // the host's briefing, a follow-up's is what was just typed.
    prompt: typedPrompt(mission, byId),
    ...(earlier.length === 0
      ? {}
      : {
          earlierTurns: earlier.map((turn) => {
            const held = byId.get(turn.missionId)
            return {
              missionId: turn.missionId,
              prompt: held === undefined ? turn.prompt : typedPrompt(held, byId),
              events: turn.events,
              peerMessages: turn.peerMessages,
              ...carriedOnOf(held),
              ...(turn.switchedFrom === undefined ? {} : { switchedFrom: turn.switchedFrom }),
              ...(turn.inTerminal === undefined ? {} : { inTerminal: turn.inTerminal }),
              ...(turn.versions === undefined ? {} : { versions: turn.versions })
            }
          })
        }),
    ...(handoff === undefined ? {} : { handoff }),
    ...(switchedFrom === undefined ? {} : { switchedFrom }),
    ...(turns.at(-1)?.versions === undefined ? {} : { versions: turns.at(-1)!.versions! })
  }
}

function restoredLiveRun(mission: PublicRecoveredMission): LiveRunState {
  const terminalError = mission.events.filter((event) => event.type === 'run.failed').at(-1)
  const error =
    mission.hostFailureMessage ??
    (terminalError?.type === 'run.failed' ? failureMessage(terminalError.payload, terminalError.sourceAdapter) : undefined) ??
    (mission.phase === 'interrupted'
      ? 'This run has no terminal receipt and was recovered as interrupted.'
      : undefined)
  return {
    prompt: mission.prompt,
    data: {
      runId: mission.runId,
      missionId: mission.missionId,
      runtime: mission.runtime,
      model: mission.model,
      resolvedRouteId: mission.resolvedRouteId,
      cliVersion: mission.cliVersion,
      sandbox: mission.sandbox
    },
    phase: mission.phase,
    events: mission.events,
    eventsTruncated: mission.eventsTruncated,
    ...(error === undefined ? {} : { error }),
    restored: true,
    restoredMission: mission,
    peerMessages: mission.peerMessages,
    ...carriedOnOf(mission),
    // A3.3: the check after this turn, kept on the record so a reopened
    // conversation still offers to send it.
    ...(mission.editCheck === undefined ? {} : { editCheck: mission.editCheck }),
    // A plan is a plan after a restart too. The ledger records what a run was
    // ALLOWED, and `ask` and `plan` are both read-only -- so until the mode
    // was recorded (schema 15) a reopened plan lost its "Build this plan"
    // offer and was told instead that its change was only in the reply, which
    // is the wrong thing to say about a plan (QA, 2026-09-06). A mission
    // recorded before 15 carries no mode and keeps the old behaviour.
    ...(mission.mode === 'plan' ? { plan: true } : {})
  }
}

/** A map with one entry replaced, or unchanged when the key is absent. */
function withRun(runs: RunMap, key: string, next: (run: LiveRunState) => LiveRunState): RunMap {
  const current = runs.get(key)
  if (current === undefined) return runs
  const copy = new Map(runs)
  copy.set(key, next(current))
  return copy
}

function withNewRun(runs: RunMap, key: string, run: LiveRunState): RunMap {
  const copy = new Map(runs)
  copy.set(key, run)
  return copy
}

function withoutRun(runs: RunMap, key: string): RunMap {
  if (!runs.has(key)) return runs
  const copy = new Map(runs)
  copy.delete(key)
  return copy
}

/**
 * The effort a mission should actually be started with. Swarm means this
 * model's maximum, so it is the last effort THIS model reported rather than a
 * fixed name some models do not have.
 */
/**
 * Who a message goes to: the teammate the person picked, and ONLY that one.
 *
 * It used to be `selectedTeammate` whenever an id was set -- and that falls
 * back to the first teammate. Removing the teammate you had picked left the id
 * set, so the composer silently addressed whoever was first in the roster, a
 * teammate the person never chose (code review B4, renderer-thread (c)). An id
 * that matches nobody now addresses nobody, which is a plain conversation --
 * the same thing the home screen sends.
 */
export function addressedTeammate<T extends { readonly teammateId: string }>(
  teammates: readonly T[],
  selectedTeammateId: string | undefined
): T | undefined {
  return selectedTeammateId === undefined ? undefined : teammates.find((teammate) => teammate.teammateId === selectedTeammateId)
}

/**
 * A Stop press, applied to a run: marked as cancelling only while it is still
 * going.
 *
 * It was unconditional (code review B4, renderer-thread (d)). A run that
 * finished between the screen being drawn and the click was re-marked
 * cancelling, the host then answered that nothing was running, and that answer
 * -- read as "the cancel failed, so it is still going" -- put it back to
 * running for good: a Stop button on a finished run that nothing would clear.
 */
export function markedCancelling(run: LiveRunState): LiveRunState {
  return liveRunIsActive(run) ? { ...run, phase: 'cancelling', error: undefined, stopPressed: true } : run
}

export function swarmEffortFor(
  models: readonly PublicModel[],
  modelId: string,
  swarm: boolean,
  chosen: string | undefined,
  runtime?: MissionRuntimeId
): string | undefined {
  const family = models.find((model) => model.id === modelId && (runtime === undefined || model.runtime === runtime))
  const supported = family?.supportedEfforts ?? []
  if (swarm) return supported[supported.length - 1]
  // The SAME fallback the chip renders (`effort ?? defaultEffort(...)` in
  // Composer.tsx), so the level on screen and the level the run is given are
  // one expression rather than two that agree by luck.
  //
  // They did not agree. `effort` starts undefined and is only assigned when
  // someone opens the dropdown, so from every launch the chip stated a level
  // -- "medium" -- while this returned undefined and the run was started with
  // no effort argument at all. The comment in status.ts claiming otherwise
  // ("because it is SET rather than merely displayed") was wrong when I wrote
  // it. Found by auditing 0.38.7's own release note, 2026-09-07.
  //
  // And the chip reads a Cursor family's level from its id before any default
  // (`effortOfShownId`): the family stands on one of its own variants, so its
  // id already names a level. Without the same step here the chip said the
  // variant's level and the run was sent `medium`.
  const ownLevel = Object.entries(family?.variants ?? {}).find(([, id]) => id === modelId)?.[0]
  return chosen ?? ownLevel ?? defaultEffort(supported, family?.defaultEffort)
}

function missionTitle(prompt: string): string {
  // The first line of a message sent with a file is the host's own "Read this
  // file in the workspace before you answer:", so every mission started with
  // an attachment was titled that -- identically, in the sidebar, in search,
  // and in the header. The title is what the person typed.
  const { text } = splitAttachments(prompt)
  const trimmed = text.trim().split('\n')[0] ?? text
  /*
   * NOT cut here. The column does the cutting.
   *
   * This used to stop at 44 characters, which CSS then ellipsised again down
   * to whatever the column was -- about 22 in the nested layout. The second
   * cut was invisible and the first one was a ceiling: widening the column
   * could never show more than 44 characters however much room it had, which
   * is why the flat list's extra 47px would have bought nothing on its own.
   *
   * The first line and the attachment split stay, because those are about
   * WHAT the title is rather than how long it may be.
   */
  return trimmed
}

/** Discovery is asked again while a runtime is still CHECKING, and on focus after this gap. */
const RUNTIME_RECHECK_MS = 15_000
/**
 * How long after an answer that crossed a finished check the held answer is
 * read again (0.639, staleChecking): long enough for the host to have stored
 * the late answer -- it does so a tick after it says the check finished.
 */
const LATE_ANSWER_REREAD_MS = 400
const RUNTIME_RECHECK_MIN_GAP_MS = 10_000
/**
 * Whether a sweep could learn anything: an installed runtime that is not
 * ready -- signed out, or not yet answered. A machine where everything
 * installed is ready has nothing a sign-in in another window could change,
 * so the sweep that ran on EVERY focus (6 s here, measured 2026-09-21) and
 * the one that ran unconditionally 15 s after launch are skipped. Check
 * again still sweeps everything.
 */
/**
 * The runtimes a re-ask is for: installed, runnable here, and not ready.
 *
 * NAMED, so the host asks only these and keeps every other answer. A re-ask
 * used to sweep everything whenever anything was unready, which with one
 * signed-out CLI was ~16 processes on every return to the window, all
 * session (main-process audit, 2026-09-22). A runtime Locust only lists
 * (`planned`) is never waited on.
 */
function unreadyRuntimes(runtimes: readonly PublicRuntimeStatus[]): readonly string[] {
  return runtimes
    .filter((entry) => entry.installed && entry.status !== 'ready' && integrationOf(entry.id) !== 'planned')
    .map((entry) => entry.id)
}

function worthAskingAgain(runtimes: readonly PublicRuntimeStatus[]): boolean {
  return unreadyRuntimes(runtimes).length > 0
}



/**
 * How many lines of npm output to keep. Enough that the end of a failing
 * install is all there -- which is where the cause is -- without letting a
 * pathological install grow in memory without end.
 */
const INSTALL_LOG_LINES = 500

export default function App(): ReactElement {
  const [runtimeState, setRuntimeState] = useState<RuntimeDiscoveryState>({ phase: 'loading' })
  const [tube, setTube] = useState<TubePreference>('full')
  /*
   * How big a reply is set. Colin's, not mine.
   *
   * 0.198.0 moved it to 18px off a character count and he saw it on his own
   * monitor: "go back to the old text size, this shit looks insane, or you
   * can have it be changeable in settings". Both, so the default is what he
   * had and the choice is on the screen.
   */
  const [replySize, setReplySize] = useState<ReplyTextSize>('standard')
  /*
   * The send button's metal, and how it behaves. Every option the design
   * agent offered is a setting rather than a constant, because Colin asked to
   * see them all before a default is picked. He picked after seeing them:
   * `silver` at `standard`, which is also what the brief asked for.
   */
  const [metal, setMetal] = useState<MetalPreset>('silver')
  const [metalStrength, setMetalStrength] = useState<MetalStrength>('standard')
  const [metalMotion, setMetalMotion] = useState<MetalMotion>('hover')
  // Every bot's screen for a face (0.561); the bots read it from botLook, not from props.
  const [terminalFaces, setTerminalFacesShown] = useState(true)
  useEffect(() => setTerminalFaces(terminalFaces), [terminalFaces])
  const [plush, setPlushShown] = useState(false)
  useEffect(() => setPlush(plush), [plush])
  const [metalBend, setMetalBend] = useState(true)
  // Declared HERE, right under its state, not a thousand lines down: a
  // helper above it closed over `runtimes` and was called during render,
  // which is a ReferenceError at boot -- and it fired only on a profile
  // that opens a finished mission from another runtime on launch (room
  // smoke, 2026-09-05), which is exactly a person's profile after an update.
  const runtimes = runtimeState.phase === 'ready' ? runtimeState.runtimes : []
  const addedAccounts = useAgentAccounts()
  // Antigravity joins a comparison through its CLI, never through its app (0.543).
  const antigravityCli = runtimes.some((status) => status.id === 'antigravity' && status.throughCli === true)
  const [build, setBuild] = useState<{ readonly version: string; readonly packaged: boolean; readonly platform: string; readonly organizationPolicy?: import('../../shared/organization-policy.js').OrganizationPolicyState }>()
  const [storage, setStorage] = useState<PublicStorageReport>()
  const [update, setUpdate] = useState<AppUpdateState>()
  /*
   * A newer Locust for this Mac (0.515): asked at launch and every six hours.
   * The host answers only on a packaged Mac build; anywhere else, nothing.
   */
  const [macRelease, setMacRelease] = useState<{ readonly version: string; readonly url: string }>()
  useEffect(() => {
    const bridge = window.desktop
    if (bridge?.macRelease === undefined || !/Macintosh|Mac OS X/.test(navigator.userAgent)) return
    const ask = (): void => {
      void bridge.macRelease().then(setMacRelease).catch(() => undefined)
    }
    ask()
    const timer = window.setInterval(ask, 6 * 60 * 60 * 1000)
    return () => window.clearInterval(timer)
  }, [])
  const [rowMenu, setRowMenu] = useState<ContextMenuState>()
  const [rowMenuArmed, setRowMenuArmed] = useState<string>()

  /**
   * The right-click menu for a mission row. Reaching an action faster is not
   * the same as skipping the question it asks, so Delete still confirms in
   * place, and a running mission cannot be deleted at all -- the host would
   * refuse it anyway, and saying so here beats an error card afterwards.
   */
  /**
   * The routine this conversation would make, if any. Read through the refs
   * because the row menu is built before the memoised history exists, and a
   * menu item that promises an action the app cannot perform is worse than
   * one that says why it is unavailable.
   */
  const routineDraftFor = (missionId: string): RoutineDraft | undefined => {
    const mission = historyByIdRef.current.get(missionId)
    if (mission === undefined) return undefined
    return routineDraft(mission, historyByIdRef.current)
  }
  /**
   * Who owns a conversation: the owner recorded on this turn, or on the
   * nearest earlier turn that has one. A conversation started with nobody
   * picked and assigned afterwards carries its owner on the turn that was
   * assigned; a follow-up typed after that is still theirs. Grok's pass 14:
   * the menu on such a follow-up said "Nothing here was typed by you" to the
   * person who had just typed it.
   */
  const conversationOwnerOf = (missionId: string): string | undefined => {
    let current: string | undefined = missionId
    for (let hop = 0; current !== undefined && hop < 64; hop += 1) {
      const owner: string | undefined = missionOwnersRef.current[current]
      if (owner !== undefined) return owner
      current = historyByIdRef.current.get(current)?.continuesFrom?.missionId
    }
    return undefined
  }

  /**
   * Right-click on a teammate. Removing one is destructive in a way a mission
   * is not -- their routines go with them -- so it asks in place like every
   * other destructive item here, and says what else it takes.
   */
  const openTeammateMenu = (teammateId: string, at: { readonly x: number; readonly y: number }): void => {
    const teammate = teammatesRef.current.find((entry) => entry.teammateId === teammateId)
    if (teammate === undefined) return
    const theirRoutines = routinesRef.current.filter((routine) => routine.teammateId === teammateId).length
    // Working right now (R22): removing them stops it, and the menu says so.
    const working = [...runsRef.current.values()].some((run) => liveRunIsActive(run) && ownerOf(run) === teammateId)
    setRowMenuArmed(undefined)
    setRowMenu({
      x: at.x,
      y: at.y,
      title: teammate.name,
      items: [
        { label: 'Message them', shortcut: 'm', onSelect: () => selectTeammate(teammateId) },
        { label: 'Edit', shortcut: 'e', onSelect: () => setEditingTeammate(teammate) },
        {
          label: 'Remove teammate',
          dividerAbove: true,
          confirmLabel: working
            ? 'Remove, and stop what they are running?'
            : theirRoutines === 0
              ? 'Remove for good?'
              : `Remove, with ${String(theirRoutines)} routine${theirRoutines === 1 ? '' : 's'}?`,
          danger: true,
          onSelect: () => removeTeammate(teammateId)
        }
      ]
    })
  }

  const startNewTeammate = (): void => {
    setTeammateError(undefined)
    setNewTeammateOpen(true)
  }
  const openRoomsScreen = (): void => {
    setRoomNotice(undefined)
    setCurrentRoomId(undefined)
    setScreen('rooms')
  }

  /*
   * THE SIDEBAR'S `+`, drawn as the right-click menus are. Colin,
   * 2026-09-24: "make the + button for new teammate and group the same style
   * as our right click dropdowns, those are way cleaner". It hangs from the
   * button; pressing the button again closes it.
   */
  const openAddMenu = (anchor: HTMLElement): void => {
    if (rowMenu?.anchor === anchor) {
      setRowMenu(undefined)
      return
    }
    const box = anchor.getBoundingClientRect()
    setRowMenuArmed(undefined)
    setRowMenu({
      x: box.left,
      y: box.bottom + 4,
      title: 'Add',
      anchor,
      items: [
        { label: 'New teammate', shortcut: 't', onSelect: startNewTeammate },
        { label: 'New room', shortcut: 'r', onSelect: openRoomsScreen },
        // Colin, 2026-09-29: "look into how claude code and codex allow you to import conversations".
        { label: 'Import a conversation…', shortcut: 'i', onSelect: () => setImportOpen(true) },
        /*
         * PROJECTS ARE BACK (Colin, 2026-09-29): "claude has projects/and
         * workspace folders, like they arent the same thing so just use that
         * setup". A project is what the person files conversations under --
         * the groups 0.460 retired, under Claude's name for them; the folder
         * a conversation works in is the chat box's chip. Not in the rail,
         * which draws no projects to name one in (M35).
         */
        ...(layoutMode === 'compact'
          ? []
          : [{
              label: 'New project',
              shortcut: 'p',
              onSelect: () => {
                setNewGroupFor(undefined)
                setNamingGroup(true)
              }
            }])
      ]
    })
  }

  const assignMissionTo = (missionId: string, teammateId: string): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    void bridge
      .assignMission(teammateId, missionId)
      .then((response) => {
        if (!response.ok) {
          setDeleteError(response.error.message)
          return
        }
        setDeleteError(undefined)
        setRowNotice(`Moved to ${teammatesRef.current.find((mate) => mate.teammateId === teammateId)?.name ?? 'that teammate'}.`)
        setMissionOwners((current) => ({ ...current, [missionId]: teammateId }))
        /*
         * And so is the box under it (0.567). Opening a conversation picks its
         * owner; handing over the one on screen did not, so the next reply
         * went to the OLD teammate -- who does not own it, so it started them
         * a new conversation. Colin, 2026-10-03, after assigning a failed
         * Antigravity conversation to Codex: "it started a whole new chat".
         */
        if (liveRunRef.current?.data?.missionId === missionId) setSelectedTeammateId(teammateId)
        // The run on screen, if it is this one, is now theirs too.
        setRuns((current) => {
          let next = current
          for (const [key, run] of current) {
            if (run.data?.missionId === missionId && run.teammateId !== teammateId) {
              next = withRun(next, key, (entry) => ({ ...entry, teammateId }))
            }
          }
          return next
        })
      })
      .catch(() => setDeleteError('That conversation could not be given to them. It still belongs to whoever had it.'))
  }

  /** Which conversation is being renamed in place, if any. */
  const [renamingMissionId, setRenamingMissionId] = useState<string>()
  // Missions shown for one teammate only, from their card (L23).
  const [missionsOwner, setMissionsOwner] = useState<string>()
  /** The Send feedback box, and the conversation it was opened from. */
  const [feedbackFor, setFeedbackFor] = useState<{ readonly conversation: string }>()

  /**
   * Name a conversation, or clear the name with an empty one.
   *
   * Keyed by the conversation -- `rootId` where there is one -- so a reply
   * cannot carry a different name from the exchange it belongs to.
   */
  const renameMission = async (missionId: string, title: string): Promise<void> => {
    const row = sidebarMissionsRef.current.find((entry) => (entry.memberIds ?? [entry.missionId]).includes(missionId))
    const key = row?.rootId ?? row?.missionId ?? missionId
    const trimmed = title.trim()
    // Optimistic, and corrected by the roster read if the host refuses: the
    // input has already closed by the time the answer lands, and a row that
    // snapped back to its old name with no word would read as a lost edit.
    setMissionTitles((current) => {
      if (trimmed.length === 0) {
        const { [key]: _gone, ...rest } = current
        return rest
      }
      return { ...current, [key]: trimmed }
    })
    const answer = await window.desktop?.renameMission(key, trimmed)
    if (answer !== undefined && !answer.ok) {
      const roster = await window.desktop?.listTeammates()
      if (roster?.ok === true) rememberRosterNames(roster.data)
    }
  }

  /** Which group is being renamed in place, if any. */
  const [renamingGroupId, setRenamingGroupId] = useState<string>()
  /** Whether the sidebar is asking for a new group's name. */
  const [namingGroup, setNamingGroup] = useState(false)
  /** The conversation whose record is being saved (0.575): any turn of it; its parents come along. */
  const [savingRecordOf, setSavingRecordOf] = useState<string>()
  /**
   * A conversation waiting for the group about to be made.
   *
   * Set only by `New group…` on a conversation's own menu, where creating a
   * group and not putting that conversation in it would be a menu item that
   * did half of what it said.
   */
  const [newGroupFor, setNewGroupFor] = useState<string>()

  const openGroupMenu = (groupId: string, at: { readonly x: number; readonly y: number }): void => {
    const group = groupsRef.current.find((entry) => entry.groupId === groupId)
    if (group === undefined) return
    setRowMenuArmed(undefined)
    setRowMenu({
      x: at.x,
      y: at.y,
      title: group.name,
      items: [
        /*
         * A menu holds actions; a dialog holds what a thing carries. The
         * group's two properties -- instructions and default route -- used to
         * be menu rows, and the route row did four jobs in one 560px strip
         * (there is a default / here it is / replace it / with this). Design
         * agent, 2026-09-16: 556px -> 208px, widest item 94px of ink.
         */
        { label: 'Project settings…', shortcut: 's', onSelect: () => setInstructingGroupId(groupId) },
        { label: 'Rename', shortcut: 'r', onSelect: () => setRenamingGroupId(groupId) },
        {
          label: 'Remove project',
          confirmLabel: 'Remove for good?',
          danger: true,
          dividerAbove: true,
          // Worth saying on the control itself: removing a container must
          // never read as removing its contents.
          onSelect: () => {
            void window.desktop?.removeGroup(groupId).then(refreshGroups)
          }
        }
      ]
    })
  }

  const openMissionMenu = (missionId: string, at: { readonly x: number; readonly y: number }): void => {
    const live = [...runsRef.current.values()].some(
      (run) => liveRunIsActive(run) && run.data?.missionId === missionId
    )
    // A run that is still STARTING has no mission id -- the host has not
    // answered with one -- so the sidebar lists it under its PENDING KEY
    // instead, deliberately, "so the teammate reads as working from the first
    // moment". Right-clicking that row and pressing Delete therefore sent a
    // key like `pending:3` to the host, which answered, correctly and
    // uselessly, "That mission does not exist" -- in a strip at the bottom of
    // the screen, far from the menu that was clicked. It read as the menu
    // doing nothing at all: Colin's third report of delete not working
    // (2026-09-06), reproduced by `_tools/drive-mission-menu.mjs` pressing it
    // and watching the row stay put.
    //
    // There is nothing to delete or hand over yet, so neither is offered, and
    // the row says which of the two it is rather than failing afterwards.
    const stillStarting = missionId.startsWith('pending:')
    const notYet = stillStarting
      ? 'This conversation is still starting. It can be changed once its first receipt lands.'
      : undefined
    const title = sidebarMissionsRef.current.find((row) => row.missionId === missionId)?.title ?? 'Conversation'
    setRowMenuArmed(undefined)
    setRowMenu({
      x: at.x,
      y: at.y,
      title,
      /*
       * Four kinds of action, a hairline between each, the way Claude's own
       * menu reads (Colin, 2026-09-22, with the two side by side): open or
       * rename it; move it or hand it over, each a list behind `›`; keep a
       * copy of it; delete it. A letter at the end of a row is a real key
       * while the menu is open.
       */
      items: [
        { label: 'Open', shortcut: 'o', onSelect: () => openMission(missionId) },
        // 0.396: watch it beside the one you are in (the side-by-side on Orca's list).
        { label: 'Open beside', shortcut: 'b', onSelect: () => setBesideId(missionId) },
        {
          label: 'Rename',
          shortcut: 'r',
          onSelect: () => setRenamingMissionId(missionId)
        },
        // Pinned to the top of the sidebar, as Claude's own list keeps its starred ones (0.729).
        {
          label: missionPinsRef.current.includes(conversationKeyOf(missionId)) ? 'Unpin' : 'Pin to top',
          shortcut: 'p',
          onSelect: () => pinConversation(missionId, !missionPinsRef.current.includes(conversationKeyOf(missionId)))
        },
        // Out of the way, or out of the way until a time; back from the same menu (0.730, settled.ts).
        ...(missionSettledRef.current[conversationKeyOf(missionId)] !== undefined
          ? [{ label: 'Bring back', shortcut: 'k', onSelect: () => settleConversation(missionId, undefined) }]
          : [
              { label: 'Settle', shortcut: 't', onSelect: () => settleConversation(missionId, {}) },
              {
                label: 'Snooze',
                submenu: snoozeChoices(new Date()).map((choice) => ({ label: choice.label, onSelect: () => settleConversation(missionId, { until: choice.until }) }))
              }
            ]),
        /*
         * ONE row that opens the list, not one row per group.
         *
         * This was a flat `Move to <name>` per group plus `Take out of
         * group`, which is the shape the menu already used for `Assign to
         * <teammate>`. It does not survive many groups: five of them buried
         * Open, Rename and Delete under five near-identical lines.
         *
         * Colin's reference, 2026-09-15, is Claude's own: one **Move to
         * group** row opening a list with a tick on the one it is already
         * in, `Ungrouped` to take it out, and `New group...` at the bottom.
         * The tick matters as much as the moving -- it makes the list answer
         * "where is this?" as well as offering to change it.
         */
        /*
         * Shown even with NO groups yet, because `New group...` lives inside
         * it -- which is the case where a person most needs it. Suppressing
         * the row until a group existed meant the only way to make a first
         * one was the `+` beside the logo, which is the Rooms
         * discoverability problem with a different noun.
         */
        ...[
              {
                label: 'Move to project',
                dividerAbove: true,
                submenu: [
                  ...groupsRef.current.map((group) => ({
                    label: group.name,
                    checked: groupOfConversation(missionId)?.groupId === group.groupId,
                    onSelect: () => {
                      void window.desktop
                        ?.assignGroup(conversationKeyOf(missionId), group.groupId)
                        .then((moved) => {
                          refreshGroups()
                          // Only the conversation on screen: seeding a route
                          // for one you are not looking at would change the
                          // composer under you.
                          const shown = liveRun?.data?.missionId ?? liveRun?.restoredMission?.missionId
                          if (moved?.ok === true && shown !== undefined && conversationKeyOf(shown) === conversationKeyOf(missionId)) {
                            seedRouteFromGroup(group)
                          }
                        })
                    }
                  })),
                  {
                    label: 'Ungrouped',
                    checked: groupMembersRef.current[conversationKeyOf(missionId)] === undefined,
                    onSelect: () => {
                      void window.desktop?.assignGroup(conversationKeyOf(missionId), undefined).then(refreshGroups)
                    }
                  },
                  // Makes the project AND files this conversation in it; named in the sidebar (M35: not in the rail).
                  ...(layoutMode === 'compact' ? [] : [{
                    label: 'New project…',
                    onSelect: () => {
                      setNewGroupFor(conversationKeyOf(missionId))
                      setNamingGroup(true)
                    }
                  }])
                ]
              }
            ],
        /*
         * Hand a conversation to a teammate after the fact: ONE row opening
         * the roster, the shape `Move to group` already has.
         *
         * It was one `Assign to <name>` row per teammate, up to six, which
         * is what made this menu tall enough to run off the bottom of the
         * window (Colin, 2026-09-22: "right click folding under window").
         * As a list it holds the whole roster at any size, and the tick on
         * whoever has it now answers "whose is this?" too.
         */
        ...(teammatesRef.current.length === 0
          ? []
          : [
              {
                label: 'Assign to',
                ...(notYet !== undefined
                  ? { disabledReason: notYet }
                  : live
                    ? { disabledReason: 'Wait for the run to finish before handing it over.' }
                    : {}),
                submenu: teammatesRef.current.map((teammate) => {
                  const owns = missionOwnersRef.current[missionId] === teammate.teammateId
                  return {
                    label: teammate.name,
                    checked: owns,
                    // Choosing whoever already has it changes nothing.
                    onSelect: () => {
                      if (!owns) assignMissionTo(missionId, teammate.teammateId)
                    }
                  }
                })
              }
            ]),
        {
          /*
           * "Save CONVERSATION as routine", because that is what it does.
           *
           * Sol's beta review of 0.225.0: opening this from one conversation
           * built a routine with TWO steps, the second being a stress task
           * that had been cancelled. The dialog was honest about it -- it
           * said "STEPS 2 OF 12", labelled the behaviour "replayed step by
           * step", and offered Remove on each -- so nothing was saved by
           * accident. But the ACTION had promised one mission and delivered
           * the thread, which is a promise the dialog then has to talk the
           * person out of.
           *
           * One word fixes the promise. Marking a cancelled step inside the
           * editor is the other half and is still open.
           */
          label: 'Save conversation as routine',
          shortcut: 's',
          dividerAbove: true,
          // Only where there is something to replay: a conversation whose
          // turns were all written by the host has no words of the person's
          // in it, and one still running has not finished the work yet.
          ...(notYet !== undefined
            ? { disabledReason: notYet }
            : live
            ? { disabledReason: 'This is still running. It can be saved when it finishes.' }
            : routineDraftFor(missionId) === undefined
              ? { disabledReason: 'Nothing here was typed by you, so there are no steps to replay.' }
              : {}),
          onSelect: () => openSaveRoutine(missionId)
        },
        {
          // The whole conversation as one file, to read before it is sent anywhere (0.575; mission-export.ts).
          label: 'Save the record…',
          shortcut: 'e',
          ...(notYet !== undefined ? { disabledReason: notYet } : {}),
          onSelect: () => setSavingRecordOf(missionId)
        },
        {
          label: 'Copy its record id',
          shortcut: 'c',
          onSelect: () => {
            void navigator.clipboard.writeText(missionId).catch(() => undefined)
          }
        },
        {
          label: 'Delete',
          shortcut: 'd',
          dividerAbove: true,
          confirmLabel: 'Delete for good?',
          danger: true,
          ...(notYet !== undefined
            ? { disabledReason: notYet }
            : live
              ? { disabledReason: 'This is still running. Stop it first.' }
              : {}),
          // Every turn the row stands for. A sidebar row is a CONVERSATION --
          // `collapseConversations` folds its turns into one line titled by
          // the first -- so deleting the id under the cursor removed only the
          // last turn and left the row on screen, which reads as the menu
          // doing nothing at all (Colin, 2026-09-05).
          onSelect: () => {
            const row = sidebarMissionsRef.current.find((entry) => entry.missionId === missionId)
            for (const turn of row?.memberIds ?? [missionId]) deleteMissionById(turn)
          }
        }
      ]
    })
  }

  const checkUpdate = async (): Promise<AppUpdateResponse> => {
    const bridge = window.desktop
    if (!bridge) return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The host is not available.' } }
    const response = await bridge.checkForUpdate()
    if (response.ok) setUpdate(response.data)
    return response
  }

  const installUpdate = async (): Promise<AppUpdateResponse> => {
    const bridge = window.desktop
    if (!bridge) return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The host is not available.' } }
    return bridge.installUpdate()
  }

  /*
   * BACK UP AND RESTORE (0.614, main/profile-backup.ts). Each answer is the
   * host's; with no host, each says so rather than pretending.
   */
  const noHost = 'Locust could not reach its own process. Restart it and try again.'
  const pickProfileFolder = async (purpose: 'backup' | 'restore'): Promise<string | undefined> =>
    window.desktop === undefined ? undefined : window.desktop.pickProfileFolder(purpose)
  const backUpProfile = async (folder: string): Promise<ProfileBackupResponse> =>
    window.desktop === undefined ? { ok: false, reason: noHost } : window.desktop.backupProfile(folder)
  const previewProfileRestore = async (folder: string): Promise<ProfileRestorePreview> =>
    window.desktop === undefined ? { ok: false, reason: noHost } : window.desktop.previewProfileRestore(folder)
  const restoreProfile = async (folder: string): Promise<ProfileRestoreResponse> =>
    window.desktop === undefined ? { ok: false, reason: noHost } : window.desktop.restoreProfile(folder)
  // What the last restore did, said once after the restart it needed; kept until dismissed.
  const [lastRestore, setLastRestore] = useState<ProfileRestoreOutcome>()
  useEffect(() => {
    void window.desktop?.takeLastProfileRestore()
      .then((outcome) => {
        if (outcome !== undefined) setLastRestore(outcome)
      })
      .catch(() => undefined)
  }, [])

  const lastPrunePreview = useRef<{ readonly days: number; readonly missionIds: readonly string[] } | undefined>(undefined)
  /** Ask the host what a prune would do. Nothing is deleted by this. */
  const previewPrune = async (days: number): Promise<MissionPruneResponse> => {
    const bridge = window.desktop
    if (!bridge) {
      return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The host is not available.' } }
    }
    const response = await bridge.pruneMissions({ olderThanDays: days, dryRun: true })
    // Remembered, so the confirm deletes what this showed and nothing else (L8).
    lastPrunePreview.current = response.ok ? { days, missionIds: response.data.deleted } : undefined
    return response
  }

  const prune = async (days: number): Promise<MissionPruneResponse> => {
    const bridge = window.desktop
    if (!bridge) {
      return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The host is not available.' } }
    }
    // Only what the preview showed: a panel left open while missions crossed
    // the cutoff, or finished, deleted those too (L8). No preview, nothing.
    const previewed = lastPrunePreview.current?.days === days ? lastPrunePreview.current.missionIds : []
    const response = await bridge.pruneMissions({ olderThanDays: days, dryRun: false, only: previewed })
    if (response.ok) {
      // A deleted mission has to leave the SCREEN as well as the disk. One
      // left open kept showing a durable receipt for a record that no longer
      // existed, and a reply to it was answered with "the earlier mission did
      // not record a session" -- which is not what happened, and the person's
      // message was never sent at all.
      const gone = new Set(response.data.deleted)
      setRuns((current) => {
        const next = new Map(current)
        for (const [key, run] of current) {
          if (run.data !== undefined && gone.has(run.data.missionId)) next.delete(key)
        }
        return next
      })
      setShownKey((current) => {
        if (current === undefined) return current
        const shown = runsRef.current.get(current)
        return shown?.data !== undefined && gone.has(shown.data.missionId) ? undefined : current
      })
      // The history and the roster changed with it. Re-read them from the
      // host rather than adjusting counts here, where they could drift.
      const [next, listed, roster] = await Promise.all([
        bridge.readStorageReport(),
        bridge.getMissionHistory(heldDigests(historyRef.current)),
        bridge.listTeammates()
      ])
      if (next.ok) setStorage(next.data)
      applyHistory(listed)
      seedLimitsFrom(listed)
      noteStore('teammates', roster.ok)
      if (roster.ok) setMissionOwners(roster.data.missionOwners)
      if (roster.ok) rememberRosterNames(roster.data)
    }
    return response
  }
  /** Every run the shell knows about, keyed by runId (or a pending key until the receipt arrives). */
  const [runs, setRuns] = useState<RunMap>(() => new Map())
  /** What the Home cover counts as work arriving (0.623): the runs going, not every rebuild of the map. */
  const coverWork = useMemo(() => workSignature(runs, isStoppable), [runs])
  /** Which run's thread is on screen; undefined shows the addressed teammate's idle state. */
  const [shownKey, setShownKey] = useState<string>()
  const [history, setHistory] = useState<readonly PublicRecoveredMission[]>([])
  /** What each CLI last said about its own setup, for its row in Settings (not the conversations). */
  const setupNotes = useMemo(() => latestSetupNotes(history), [history])
  /** The history as last committed, for a read to say what it already holds (historyMerge.ts). */
  const historyRef = useRef<readonly PublicRecoveredMission[]>([])
  useEffect(() => {
    historyRef.current = history
  }, [history])
  /** Ledger files that raised an issue and yielded no mission. See the history read. */
  const [unreadableLedgers, setUnreadableLedgers] = useState(0)
  const [totalMissions, setTotalMissions] = useState<{ readonly total: number; readonly listed: number } | undefined>(undefined)
  /** The ledger could not be read AT ALL -- not the same as having no missions. */
  const [ledgerUnreadable, setLedgerUnreadable] = useState(false)

  /**
   * Apply a history response to ALL of what it decides, in one place.
   *
   * Three separate reads call this: the initial load, `refreshHistory`, and
   * the re-read after a storage prune. Each of them used to call `setHistory`
   * on its own, and only the first was taught about the two damage states -- so
   * pruning the damaged mission, or any later refresh, left a warning on
   * screen that its own data no longer supported, and a ledger that BECAME
   * unreadable after launch would never have said so.
   *
   * Raised by Astra as a source-only concern, 2026-09-09: "App's other history
   * reads update missions but not these two new damage states." Not
   * live-reproduced, and it does not need to be -- three call sites deciding
   * two related things independently is the shape of the bug, not evidence of
   * one.
   */
  const applyHistory = (response: MissionHistoryResponse): void => {
    if (!response.ok) {
      // A failed read is not an empty ledger. `history` is deliberately left
      // as it was: replacing known missions with [] on a transient read
      // failure would erase the record from the screen.
      setLedgerUnreadable(true)
      return
    }
    setLedgerUnreadable(false)
    // What the host kept back is what this window already holds: the same
    // objects, so nothing built from them is built again.
    setHistory((current) => mergeHistory(current, response.data.missions))
    setRuns((current) => recoverLiveRuns(
      current, response.data.missions, response.data.liveMissionIds ?? [], pendingUpdatesRef.current,
      (mission) => reopenedRun(mission, new Map(response.data.missions.map((one) => [one.missionId, one]))), applyMissionUpdate
    ))
    setUnreadableLedgers(response.data.unreadableCount)
    setTotalMissions(response.data.totalMissions === undefined || response.data.listedMissions === undefined ? undefined : { total: response.data.totalMissions, listed: response.data.listedMissions })
  }
  const [teammates, setTeammates] = useState<readonly PublicTeammate[]>([])
  const [missionOwners, setMissionOwners] = useState<Readonly<Record<string, string>>>({})
  const [missionTitles, setMissionTitles] = useState<Readonly<Record<string, string>>>({})
  /** Conversations pinned to the sidebar's top, by conversation key, newest pin first (0.729). */
  const [missionPins, setMissionPins] = useState<readonly string[]>([])
  /** Conversations settled or snoozed out of the sidebar, by conversation key (0.730). */
  const [missionSettled, setMissionSettled] = useState<Readonly<Record<string, SettledEntry>>>({})
  const missionSettledRef = useRef<Readonly<Record<string, SettledEntry>>>({})
  missionSettledRef.current = missionSettled
  /** What the roster says about conversations: their typed names, and which are pinned. */
  const missionPinsRef = useRef<readonly string[]>([])
  missionPinsRef.current = missionPins
  /** Pin or unpin a conversation, by its key: shown at once, kept by the host, put back if it refuses. */
  const pinConversation = (missionId: string, pinned: boolean): void => {
    const key = conversationKeyOf(missionId)
    const before = missionPinsRef.current
    setMissionPins(pinned ? [key, ...before.filter((id) => id !== key)] : before.filter((id) => id !== key))
    void window.desktop
      ?.pinMission(key, pinned)
      .then((answer) => {
        if (!answer.ok) {
          setMissionPins(before)
          setTeammateError(answer.error.message)
        }
      })
      .catch(() => setMissionPins(before))
  }
  const rememberRosterNames = (data: {
    readonly missionTitles: Readonly<Record<string, string>>
    readonly missionPins?: readonly string[]
    readonly missionSettled?: Readonly<Record<string, SettledEntry>>
  }): void => {
    setMissionTitles(data.missionTitles)
    setMissionPins(data.missionPins ?? [])
    setMissionSettled(data.missionSettled ?? {})
  }
  /** Settle a conversation, snooze it until a time, or bring it back: shown at once, put back if refused (0.730). */
  const settleConversation = (missionId: string, settled: { readonly until?: string } | undefined): void => {
    const key = conversationKeyOf(missionId)
    const before = missionSettledRef.current
    const { [key]: _was, ...rest } = before
    setMissionSettled(settled === undefined ? rest : { ...rest, [key]: settled.until === undefined ? { at: new Date().toISOString() } : { at: new Date().toISOString(), until: settled.until } })
    void window.desktop
      ?.settleMission(key, settled)
      .then((answer) => {
        if (!answer.ok) {
          setMissionSettled(before)
          setTeammateError(answer.error.message)
        }
      })
      .catch(() => setMissionSettled(before))
  }
  const [groups, setGroups] = useState<readonly PublicGroup[]>([])
  const [groupMembers, setGroupMembers] = useState<Readonly<Record<string, GroupMembership>>>({})
  const [groupLeft, setGroupLeft] = useState<Readonly<Record<string, readonly LeftMembership[]>>>({})
  const [changelog, setChangelog] = useState<AppChangelog | undefined>(undefined)
  /**
   * Local files that exist and refused to read, by store name. Held apart
   * from "empty" so the sidebar can say so -- an unreadable roster used to
   * look exactly like a fresh install.
   */
  const [unreadableStores, setUnreadableStores] = useState<readonly string[]>([])
  /**
   * A group's default route becomes the composer's pending selection when a
   * conversation joins it -- the ruling's "seeding it on join rewrites
   * nothing": a route is recorded per mission, so this is the selection for
   * the NEXT turn and nothing about any turn already run.
   */
  const seedRouteFromGroup = (group: PublicGroup): void => {
    if (group.route === undefined) return
    const next = { runtime: group.route.runtime, model: group.route.model, ...(group.route.account === undefined ? {} : { account: group.route.account }) }
    /*
     * The same three writes the composer's own route picker makes, or the
     * seed does not show. MEASURED 2026-09-16 driving 0.161.0 before release:
     * `setRoute` alone left the composer reading "Codex / Account Default"
     * after the join, because what the composer draws is
     * `composerRouteFor(route, pickedTeammate, pickerRoutes)` -- the picked
     * teammate's own entry wins over the bare route state. A seed that only
     * wrote the bare state was overruled on the next render.
     */
    routeChosen.current = true
    setRoute(next)
    setPickerRoutes((current) => new Map(current).set(pickKey, next))
    setMode(group.route.mode)
    // Named, not spread: `undefined` clears an effort the group's runtime has no levels for.
    setEffort(group.route.effort)
  }
  const noteStore = (name: string, ok: boolean): void => {
    setUnreadableStores((held) => (ok ? held.filter((entry) => entry !== name) : held.includes(name) ? held : [...held, name]))
  }
  const refreshGroups = (): void => {
    void window.desktop
      ?.listGroups()
      .then((response) => {
        noteStore('groups', response.ok)
        if (!response.ok) return
        setGroups(response.data.groups)
        setGroupMembers(response.data.members)
        setGroupLeft(response.data.left)
      })
      .catch(() => undefined)
  }
  /**
   * Replies the host has decided on and is still starting, by teammate.
   *
   * The seconds between deciding on a hop and the runtime existing used to be
   * silent: `mission-started` needs a run id, so a cold start left every row
   * idle and a room mid-argument looked finished. One entry per teammate,
   * because a teammate runs one mission at a time.
   */
  const [relayStarting, setRelayStarting] = useState<Readonly<Record<string, StartingReply>>>({})
  const [newTeammateOpen, setNewTeammateOpen] = useState(false)
  /** The group whose settings -- instructions and default route -- are open, if any. */
  const [instructingGroupId, setInstructingGroupId] = useState<string>()
  // Import a conversation from Claude Code or Codex (ImportDialog, session-import.ts).
  const [importOpen, setImportOpen] = useState(false)
  // Usage across every agent and model (UsageDialog, 0.714), opened from its line on Home.
  const [usageOpen, setUsageOpen] = useState(false)
  // The conversation just imported, opened once the history holds it.
  const openAfterImport = useRef<string | undefined>(undefined)
  const [routines, setRoutines] = useState<readonly PublicRoutine[]>([])
  /** Rooms: a named set of teammates a person writes to at once (vision #2). */
  const [rooms, setRooms] = useState<readonly PublicRoom[]>([])
  const [currentRoomId, setCurrentRoomId] = useState<string>()
  const [roomNotice, setRoomNotice] = useState<string>()
  /** What the team remembers, as the host last listed it. */
  const [memories, setMemories] = useState<readonly PublicMemory[]>([])
  const [forgottenMemories, setForgottenMemories] = useState<readonly PublicForgottenMemory[]>([])
  const [memoriesOutOfDate, setMemoriesOutOfDate] = useState<Readonly<Record<string, readonly string[]>>>({})
  const [briefTrackingSince, setBriefTrackingSince] = useState<string>()
  const [memoryWorkspace, setMemoryWorkspace] = useState<{ readonly id: string; readonly name: string }>({ id: '', name: '' })
  const [memoryMode, setMemoryMode] = useState<MemoryMode>(DEFAULT_MEMORY_MODE)
  /**
   * Which shell layout to draw, and how wide the window is.
   *
   * The width is tracked because `auto` -- the default -- answers with it, and
   * a resize has to redraw. `resolveLayout` is the only place the two are
   * combined; see `layout.ts` for why this is not a media query any more.
   */
  const [layout, setLayout] = useState<LayoutPreference>('auto')
  const [windowWidth, setWindowWidth] = useState<number>(() =>
    typeof window === 'undefined' ? 1440 : window.innerWidth
  )
  useEffect(() => {
    const onResize = (): void => setWindowWidth(window.innerWidth)
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [])
  const layoutMode = resolveLayout(layout, windowWidth)
  /** The boot screen's own setting, persisted the way the layout one is. */
  const chooseTube = (next: TubePreference): void => {
    const before = tube
    setTube(next)
    void window.desktop
      ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube: next })
      .then((settings) => setTube(settings.tube))
      .catch(() => setTube(before))
  }
  /**
   * One writer for all four, because they are one decision in four parts and
   * writing them separately would mean four round trips to change a look.
   */
  const chooseMetal = (next: {
    readonly metal?: MetalPreset
    readonly metalStrength?: MetalStrength
    readonly metalMotion?: MetalMotion
    readonly metalBend?: boolean
  }): void => {
    const before = { metal, metalStrength, metalMotion, metalBend }
    const wanted = { ...before, ...next }
    setMetal(wanted.metal)
    setMetalStrength(wanted.metalStrength)
    setMetalMotion(wanted.metalMotion)
    setMetalBend(wanted.metalBend)
    void window.desktop
      ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube, ...wanted })
      .then((settings) => {
        setMetal(settings.metal ?? wanted.metal)
        setMetalStrength(settings.metalStrength ?? wanted.metalStrength)
        setMetalMotion(settings.metalMotion ?? wanted.metalMotion)
        setMetalBend(settings.metalBend ?? wanted.metalBend)
      })
      // Put it back rather than leave the screen claiming a setting the disk
      // never took -- the same rule every other writer here follows.
      .catch(() => {
        setMetal(before.metal)
        setMetalStrength(before.metalStrength)
        setMetalMotion(before.metalMotion)
        setMetalBend(before.metalBend)
      })
  }
  /** Terminal faces on or off: shown at once, put back if the disk did not take it. */
  const chooseTerminalFaces = (next: boolean): void => {
    const before = terminalFaces
    setTerminalFacesShown(next)
    void window.desktop
      ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube, terminalFaces: next })
      .then((settings) => setTerminalFacesShown(settings.terminalFaces !== false))
      .catch(() => setTerminalFacesShown(before))
  }
  /** Plush on or off: shown at once, put back if the disk did not take it (0.577). */
  const choosePlush = (next: boolean): void => {
    const before = plush
    setPlushShown(next)
    void window.desktop
      ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube, plush: next })
      .then((settings) => setPlushShown(settings.plush === true))
      .catch(() => setPlushShown(before))
  }
  // A3.3: this folder's check after edits, as the host last said it.
  const [checkCommand, setCheckCommand] = useState('')
  const [editChecks, setEditChecks] = useState<ReadonlyMap<string, EditCheckShown>>(new Map())
  const saveCheckCommand = (next: string): void => {
    void window.desktop
      ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube, checkCommand: next })
      .then((settings) => setCheckCommand(settings.checkCommand ?? ''))
      .catch(() => undefined)
  }
  /*
   * About you (0.423): the person's standing note, read by every teammate
   * (shared/about-you.ts). Saved whole; '' removes it. Answers an error
   * sentence or undefined, as the Memory screen's other writes do.
   */
  /*
   * Each runtime's own slash commands (0.426), as its CLI last listed them.
   * Read once, and again when a run brings a changed list.
   */
  const [runtimeCommands, setRuntimeCommands] = useState<RuntimeCommandsResponse>({})
  const rereadRuntimeCommands = (): void => {
    void window.desktop?.runtimeCommands().then(setRuntimeCommands).catch(() => undefined)
  }
  useEffect(() => { rereadRuntimeCommands() }, [])
  const [aboutYou, setAboutYou] = useState('')
  // Lines teammates suggested for it, waiting for the person (0.424).
  const [aboutYouSuggestions, setAboutYouSuggestions] = useState<readonly AboutYouSuggestion[]>([])
  const rereadAboutYou = (): void => {
    void window.desktop?.readWorkspaceSettings().then((settings) => {
      setAboutYou(settings.aboutYou ?? '')
      setAboutYouSuggestions(settings.aboutYouSuggestions ?? [])
    }).catch(() => undefined)
  }
  /** Add a suggested line to the note, or let it go. Either way it stops waiting. */
  const answerAboutYou = async (id: string, add: boolean): Promise<string | undefined> => {
    const bridge = window.desktop
    if (!bridge) return 'Locust is not ready yet. Nothing was changed.'
    const suggestion = aboutYouSuggestions.find((entry) => entry.id === id)
    if (suggestion === undefined) return undefined
    const rest = aboutYouSuggestions.filter((entry) => entry.id !== id)
    try {
      const settings = await bridge.writeWorkspaceSettings({
        swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube,
        ...(add ? { aboutYou: withSuggestion(aboutYou, suggestion.text) } : {}),
        aboutYouSuggestions: rest
      })
      setAboutYou(settings.aboutYou ?? '')
      setAboutYouSuggestions(settings.aboutYouSuggestions ?? [])
      return undefined
    } catch {
      return 'That could not be saved. Your note is unchanged.'
    }
  }
  const saveAboutYou = async (next: string): Promise<string | undefined> => {
    const bridge = window.desktop
    if (!bridge) return 'Locust is not ready yet. Nothing was saved.'
    // Said here, in its own words; the store refuses it as well (0.433).
    const secret = secretIn(next)
    if (secret !== undefined) return aboutYouSecretRefusal(secret)
    try {
      const settings = await bridge.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube, aboutYou: next })
      setAboutYou(settings.aboutYou ?? '')
      return undefined
    } catch {
      return 'Your note could not be saved. What your teammates read is unchanged.'
    }
  }
  const chooseReplySize = (next: ReplyTextSize): void => {
    const before = replySize
    setReplySize(next)
    void window.desktop
      ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize: next, layout, tube })
      .then((settings) => setReplySize(settings.replySize))
      .catch(() => setReplySize(before))
  }
  /*
   * APPLIED ON THE ROOT, because the size it changes is a token and tokens
   * live on :root. One attribute, read by two rules in tokens.css -- the
   * same shape the shell class for the inspector uses, and nothing here
   * computes a pixel value: the stylesheet owns what each choice means.
   */
  useEffect(() => {
    document.documentElement.dataset.replysize = replySize
  }, [replySize])
  const chooseLayout = (next: LayoutPreference): void => {
    const before = layout
    setLayout(next)
    void window.desktop
      ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout: next, tube })
      .then((settings) => setLayout(isLayoutPreference(settings.layout) ? settings.layout : 'auto'))
      .catch(() => setLayout(before))
  }
  const [memoryNotice, setMemoryNotice] = useState<string>()
  /** The notice points at suggestions waiting on the Memory screen, so it goes once none is waiting. */
  const [memoryNoticeWaits, setMemoryNoticeWaits] = useState(false)
  /**
   * The last scheduled routine that would not start, and why. Kept until it
   * is read: a routine that stops happening on a schedule is exactly the
   * thing a person is not watching for.
   */
  const [automationNotice, setAutomationNotice] = useState<string>()
  /** Each runtime's own MCP servers and hooks, read once at boot and again when Settings opens. */
  const [runtimeSetup, setRuntimeSetup] = useState<Readonly<Record<string, PublicRuntimeSetup>>>()
  const [workspaceBrief, setWorkspaceBrief] = useState<PublicWorkspaceBrief | null>()
  const [worktrees, setWorktrees] = useState<{ readonly list: readonly PublicWorktree[]; readonly reason: string | undefined }>()
  /**
   * What a person typed while a mission was running, waiting to go as the
   * next turn. Kept against the RUN it was typed at, not the teammate, so it
   * can never be sent into some other conversation: if that run is no longer
   * the one on screen, it waits and says so rather than guessing.
   */
  /*
   * Everything typed while a teammate is working, in order.
   *
   * One slot until 2026-09-13, so a second thought REPLACED the first -- the
   * person watched their own words disappear. It is a list now, and the
   * leading run of plain follow-ups is folded into one turn when it goes, so
   * three thoughts reach the teammate as one instruction it can answer
   * knowing all three, rather than as three turns it answers blind. Gates and
   * reasoning in `steering.ts`; grok-build does the same thing and Colin
   * asked for it by name.
   */
  const [queued, setQueued, queueError, sendQueued] = useConversationQueue()
  /** The save/edit dialog, open on a draft taken from a conversation or on a routine already saved. */
  const [routineDialog, setRoutineDialog] = useState<{
    readonly inputs?: readonly RoutineInput[]
    /** Undefined for a conversation nobody owns: the dialog asks who will run it. */
    readonly teammateId: string | undefined
    readonly routineId?: string
    readonly name: string
    readonly steps: readonly string[]
    readonly learnedFrom: readonly string[]
    readonly truncated: boolean
    readonly route?: TeammateRoute
    readonly schedule?: RoutineSchedule
    /** Who takes each step (0.435), when a saved routine is edited. */
    readonly handOffs?: readonly RoutineHandOff[]
    /** A saved routine's own mode, and whether a run of it is going (0.493). */
    readonly savedMode?: MissionMode
    readonly running?: boolean
    /** It works in a copy, kept or discarded by the person (0.533). */
    readonly inCopy?: boolean
    /** Its standing goal's fixes, when it has one (0.534). */
    readonly goalTries?: number
    /** A chain template opened it set to change files (2026-10-05): saved so, unless the person moves it. */
    readonly forceMode?: MissionMode
    /** The role a chain template named for each step, shown beside who was proposed. */
    readonly stepRoles?: readonly (string | undefined)[]
    readonly busy: boolean
    readonly error?: string
  }>()
  /** The teammate being edited in the same dialog, when it is open for editing. */
  const [editingTeammate, setEditingTeammate] = useState<PublicTeammate>()
  /** Why a folder request for one teammate did nothing. */
  const [folderNotice, setFolderNotice] = useState<string>()
  const [inspectorOpen, setInspectorOpen] = useState(false)
  /**
   * COMPARE (0.441, shared/compare.ts): the saved comparisons, the one on
   * screen, and the models ticked in the picker for the next ask.
   */
  const [compares, setCompares] = useState<readonly PublicCompare[]>([])
  const [comparingId, setComparingId] = useState<string>()
  /** The comparison on screen was opened from Benchmarks (0.764): its Back goes there. */
  const [backToBenchmarks, setBackToBenchmarks] = useState(false)
  /** The comparison whose benchmark is scrolled to as it opens: opened from Benchmarks, or back from one of its runs. */
  const [revealBenchmarkOf, setRevealBenchmarkOf] = useState<string>()
  useEffect(() => {
    if (comparingId === undefined) setBackToBenchmarks(false)
    if (comparingId !== revealBenchmarkOf) setRevealBenchmarkOf(undefined)
  }, [comparingId])
  const [compareOn, setCompareOn] = useState(false)
  /** The next comparison edits: each model changes its own copy (0.445). */
  const [compareChanges, setCompareChanges] = useState(false)
  /**
   * Why a comparison in this folder cannot change files -- a plain folder too
   * big to copy -- asked of the host when Compare opens (0.457). Auto is then
   * greyed out in the Compare menu with this as its reason, and a comparison
   * already set to change files goes back to answering.
   */
  const [compareChangesRefusal, setCompareChangesRefusal] = useState<string>()
  const [compareInPlace, setCompareInPlace] = useState(false)
  /** The next comparison hides the names until one is kept (0.449). */
  const [compareBlind, setCompareBlind] = useState(false)
  /** What each column of the comparison on screen has changed, when it edits. */
  const [compareChangeLines, setCompareChangeLines] = useState<{ readonly compareId: string; readonly columns: Partial<Record<CompareSlotId, string>> }>()
  const [comparePicks, setComparePicks] = useState<readonly ComparePick[]>([])
  const [keepingCompare, setKeepingCompare] = useState(false)
  /** The column being asked again (0.444), while it starts. */
  const [retryingCompare, setRetryingCompare] = useState<CompareSlotId | undefined>(undefined)
  // A judge being asked, while its run is started (0.520).
  const [judgingCompare, setJudgingCompare] = useState(false)
  // Another model being added to a comparison (0.523).
  const [addingCompareModel, setAddingCompareModel] = useState(false)
  // Your own benchmark (0.763): where each comparison's runs again are, a run again being asked for, a model being set.
  const [rerunProgress, setRerunProgress] = useState<readonly RerunProgress[]>([])
  const [startingRerun, setStartingRerun] = useState(false)
  const [usingBenchmarkRow, setUsingBenchmarkRow] = useState<string>()
  const [benchmarkProblem, setBenchmarkProblem] = useState<string>()
  useEffect(() => {
    void window.desktop?.compareReruns?.().then(setRerunProgress).catch(() => undefined)
  }, [])
  // Every judge's run, drawn in its comparison (0.520), and every look of Scout's, whose answer is cards on Home
  // (0.756): never listed as a conversation.
  const scout = useScoutState()
  const scoutLookKey = (scout?.lookIds ?? []).join(' ')
  // A comparison's runs again (0.763) too: the comparison they run again shows them, as its benchmark.
  // And the runs whose answer belongs to a panel (0.771): a commit message, a walk through a change.
  const [panelRunIds, setPanelRunIds] = useState<readonly string[]>([])
  useEffect(() => {
    void window.desktop?.panelRuns?.().then(setPanelRunIds).catch(() => undefined)
  }, [])
  const hiddenRunIds = useMemo(
    () => new Set([...judgeMissionIds(compares), ...rerunMissionIds(compares), ...(scoutLookKey.length === 0 ? [] : scoutLookKey.split(' ')), ...panelRunIds]),
    [compares, scoutLookKey, panelRunIds]
  )
  const rerunCompareIds = useMemo(() => new Set(compares.flatMap((compare) => (compare.rerunOf === undefined ? [] : [compare.compareId]))), [compares])
  const isHiddenRun = (startedBy: { readonly kind: string } | undefined, missionId: string | undefined): boolean =>
    startedBy?.kind === 'judge' || startedBy?.kind === 'scout' || isPanelRunKind(startedBy?.kind) || (missionId !== undefined && hiddenRunIds.has(missionId)) ||
    (startedBy?.kind === 'compare' && rerunCompareIds.has((startedBy as { readonly compareId?: string }).compareId ?? ''))
  const [compareProblem, setCompareProblem] = useState<string>()
  /** Home's Compare models opens the composer's picker (0.442). */
  const [pickerRequest, setPickerRequest] = useState(0)
  /** Words put in the message box from outside it (0.448: a Build and compare starter; 0.770: a link). */
  const [composerFill, setComposerFill] = useState<{ readonly text: string; readonly seq: number; readonly from?: 'link' }>()
  /**
   * An earlier message being edited (0.498, Claude Code's rewind): which
   * conversation on screen (`key`), its latest turn (`tip`), the turn before
   * the edited one (`before`, absent for the first message) and the turns up
   * to it, which the new branch is drawn under.
   */
  const [rewinding, setRewinding] = useState<{
    readonly key: string
    readonly tip: string
    readonly before?: string
    readonly earlierTurns: NonNullable<LiveRunState['earlierTurns']>
    /** What the replies after it changed (0.502), and whether to put it back. Off unless ticked. */
    readonly files: readonly FilePutBack[]
    readonly setAside: readonly string[]
    readonly putBack: boolean
  }>()
  useEffect(() => {
    void window.desktop?.listCompares().then((answer) => {
      if (answer.ok) setCompares(answer.data.compares)
    }).catch(() => undefined)
  }, [])
  /** Review changes (0.439): the teammate whose own branch is open beside the thread. */
  const [reviewingId, setReviewingId] = useState<string>()
  /** The folder's changes, in one panel beside the conversation (0.732, ChangesPanel.tsx). */
  const [changesOpen, setChangesOpen] = useState(false)
  const [terminalOpen, setTerminalOpen] = useState(false)
  const [terminalMounted, setTerminalMounted] = useState(false)
  const [browserOpen, setBrowserOpen] = useState(false)
  const [browserMounted, setBrowserMounted] = useState(false)
  const terminalRefusal = policyTerminalRefusal(build?.organizationPolicy)
  const browserRefusal = policyBrowserRefusal(build?.organizationPolicy)
  const closeBrowserPanel = () => {
    setBrowserOpen(false)
    void window.desktop?.browser?.closePanel().catch(() => undefined)
  }
  const toggleTerminal = () => {
    if (terminalRefusal !== undefined) return
    setTerminalMounted(true)
    setTerminalOpen(open => !open)
    closeBrowserPanel()
    setScreen('workroom')
    setChangesOpen(false)
    setInspectorOpen(false)
    setReviewingId(undefined)
    setBesideId(undefined)
    setViewingFile(undefined)
    setViewerRefusal(undefined)
    setSideChat(undefined)
    setCloudPanel(false)
    setBackgroundPanel(false)
  }
  const toggleBrowser = () => {
    if (browserRefusal !== undefined) return
    setBrowserMounted(true)
    const nextOpen = !browserOpen
    setBrowserOpen(nextOpen)
    setTerminalOpen(false)
    setScreen('workroom')
    setChangesOpen(false)
    setInspectorOpen(false)
    setReviewingId(undefined)
    setBesideId(undefined)
    setViewingFile(undefined)
    setViewerRefusal(undefined)
    setSideChat(undefined)
    setCloudPanel(false)
    setBackgroundPanel(false)
    if (nextOpen) {
      void window.desktop?.browser?.openPanel().catch(() => undefined)
    } else {
      void window.desktop?.browser?.closePanel().catch(() => undefined)
    }
  }
  useEffect(() => {
    const api = window.desktop?.browser
    if (api === undefined) return
    return api.onState((next) => {
      if (next.open) {
        setBrowserMounted(true)
        setBrowserOpen(true)
        setTerminalOpen(false)
        setChangesOpen(false)
        setInspectorOpen(false)
        setReviewingId(undefined)
        setBesideId(undefined)
        setViewingFile(undefined)
        setViewerRefusal(undefined)
        setSideChat(undefined)
        setCloudPanel(false)
        setBackgroundPanel(false)
      }
    })
  }, [])
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!isTerminalToggle(event) || event.repeat) return
      event.preventDefault()
      toggleTerminal()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [terminalRefusal])
  // Commits made from the Commit button: the Changes panel reads the folder again on each.
  const [commits, setCommits] = useState(0)
  /*
   * The file open beside the conversation, if any.
   *
   * It shares the inspector's REGION and not its state: somebody reading a
   * report does not want the mission inspector's tabs underneath it, and the
   * inspector's own toggle should not close their file. The slot below draws
   * whichever is open, with the file winning -- it was opened by a press on a
   * specific thing, which is a more specific intent than a toggle.
   */
  /*
   * A SECOND CONVERSATION, BESIDE THE ONE ON SCREEN (0.396).
   *
   * Orca's list, item 4: two agents side by side. Read-only on purpose --
   * the chat box stays with the conversation you are in, so there is never
   * a question which one a message goes to -- and live: a run still going
   * streams into it. Any turn of the conversation names it; the newest is
   * what is drawn.
   */
  const [besideId, setBesideId] = useState<string>()
  /**
   * Ask on the side (0.461, SideChat.tsx): the conversation turn it asks
   * about, the side runs asked so far (by run id, oldest first), and the
   * model that answers -- the conversation's own.
   */
  const [sideChat, setSideChat] = useState<{ readonly of: string; readonly runIds: readonly string[]; readonly model: string }>()
  const [tuning, setTuning] = useState<TuneCard>()
  const tuneGeneration = useRef(0)
  useEffect(() => {
    if (tuning?.draftRunId === undefined || !tuning.drafting) return
    const run = runs.get(tuning.draftRunId)
    if (run === undefined || !isTerminal(run.phase)) return
    const raw = run.phase === 'completed' ? assistantMessages(run.events).filter((message) => message.final).at(-1)?.text ?? '' : ''
    setTuning((current) => current?.id !== tuning.id ? current : { ...current, drafting: false, drafts: parseTuneDrafts(raw, current.current) })
  }, [runs, tuning])
  /*
   * CLOUD TASKS (0.503, main/cloud-tasks.ts): Cloud picked in the chat-type
   * menu, this folder's tasks, and the panel beside the conversation.
   */
  const [cloudOn, setCloudOn] = useState(false)
  /** Claude Code or Codex on the person's own cloud computer (0.768), instead of its maker's cloud. */
  const [computerOn, setComputerOn] = useState(false)
  /** Ctrl+F, counted (0.769): the conversation on screen opens its find bar on each new number. */
  const [findRequest, setFindRequest] = useState(0)
  // YOUR CLOUD COMPUTER (0.761): the turns that ran there, by mission, and the first-time dialog.
  const [cloudTurns, setCloudTurns] = useState<ReadonlyMap<string, PublicCloudTurn>>(() => new Map())
  const [cloudSetup, setCloudSetup] = useState<CloudSetupState>()
  /** Background (W10): picked in the chat-type menu, the runs, the panel, and why the last send did not start. */
  const [backgroundOn, setBackgroundOn] = useState(false)
  const [backgroundRuns, setBackgroundRuns] = useState<readonly PublicBackgroundRun[]>([])
  const [backgroundPanel, setBackgroundPanel] = useState(false)
  const [backgroundNote, setBackgroundNote] = useState<{ readonly text: string; readonly setUp?: true }>()
  const backgroundTurnIds = useMemo(() => new Set(backgroundRuns.flatMap((run) => (run.broughtIn === undefined ? [] : [run.broughtIn]))), [backgroundRuns])
  const [cloudTasks, setCloudTasks] = useState<readonly PublicCloudTask[]>([])
  const [cloudPanel, setCloudPanel] = useState(false)
  // Claude's cloud (0.538): what was sent from this folder, to find again or bring home.
  const [claudeCloud, setClaudeCloud] = useState<readonly PublicClaudeCloudSession[]>([])
  const [claudeCloudNote, setClaudeCloudNote] = useState<string>()
  // The header's folder button, when the folder could not be opened (0.797): said on its hover.
  const [folderProblem, setFolderProblem] = useState<string>()
  const [cloudWhere, setCloudWhere] = useState<PublicCloudWhere>()
  const [cloudNotes, setCloudNotes] = useState<readonly string[]>([])
  const [cloudProblem, setCloudProblem] = useState<string>()
  const [cloudApplying, setCloudApplying] = useState<string>()
  // Undefined while they are being looked for: each is asked whether it has a cloud environment.
  const [cloudFolders, setCloudFolders] = useState<readonly PublicCloudFolder[] | undefined>([])
  const [viewingFile, setViewingFile] = useState<{
    readonly path: string
    /** The file's text, or a `data:` URL when the mode is `image`. */
    readonly text: string
    readonly mode: 'markdown' | 'code' | 'image' | 'table' | 'document' | 'pdf'
    /** A PDF's page pictures, when the mode is `pdf` (0.714). */
    readonly pdf?: { readonly pages: number; readonly pictures: readonly string[]; readonly folder: string }
    /** A web page's address in the preview frame (0.425); its text stays the source. */
    readonly pageUrl?: string
    /** A spreadsheet's cells, when the mode is `table` (0.364). */
    readonly workbook?: Workbook
    /** A Word or PowerPoint file's words, when the mode is `document` (0.517). */
    readonly document?: OfficeDocument
  }>()
  /** What the host said when a file could not be opened. Shown where the press was. */
  const [viewerRefusal, setViewerRefusal] = useState<string>()
  // Which file it was, for the refusal's heading (0.524): "Could not open that file" named none.
  const [refusedName, setRefusedName] = useState<string>()
  const openFileInViewer = (path: string, imageFolder = workspacePath): void => {
    const bridge = window.desktop
    setViewerRefusal(undefined)
    setRefusedName(path.replace(/[\\/]+$/, '').split(/[\\/]/).pop())
    if (bridge === undefined || workspacePath === undefined) return
    /*
     * Two callers, two shapes of path.
     *
     * A handed file is relative to the folder, or whole since 0.787 (a file
     * written elsewhere, handed over by its full path). An activity row's file is absolute,
     * because that is what the runtime reported acting on, and it is not
     * always inside the workspace. Joining the workspace onto an already
     * absolute path produces `C:/ws/C:/elsewhere/notes.md`, which the host
     * then refuses for the wrong reason: the panel would say the file is
     * outside the folder about a file that is simply named twice.
     */
    const absolute = /^([a-z]:)?[\\/]/i.test(path)
    // Resolve ONCE and keep the resolved path. The head's reveal and save
    // buttons used to join the workspace on again, so whichever caller had
    // already passed a whole path got it joined twice at the moment it was
    // pressed -- a panel showing the right file with two controls pointing at
    // a path that does not exist.
    const full = absolute ? path : `${workspacePath}/${path}`
    /*
     * A PICTURE IS SHOWN AS A PICTURE.
     *
     * A teammate that makes a chart, a diagram or a screenshot produces a
     * PNG, and until now pressing it got "Locust does not open that kind of
     * file here" -- a refusal, about a file the teammate had just made for
     * you. `isViewableText` is an allowlist of text, correctly, and an image
     * is simply not text.
     *
     * This is the half of "artifact support" that costs nothing to give
     * (`docs/DECISION-2026-09-20-LOCUST-NEVER-RUNS-MODEL-CODE.md`): drawing a
     * raster image executes nothing. The plumbing is the attachments'
     * already, including the part that matters -- **SVG is deliberately not
     * painted**, because an SVG is a document that can carry script. It stays
     * text, and opens as code, which is the honest way to show one.
     */
    /*
     * A PDF IS SHOWN AS ITS PAGES (0.714): the pictures Locust drew of it for
     * the agent, read on the way for one attached before there were any.
     * Pictures, so nothing in the file runs -- the viewer never launches one.
     */
    if (/.pdf$/i.test(full)) {
      const folder = imageFolder ?? workspacePath
      void bridge
        .pdfPages(full, folder)
        .then((answer) => {
          if (answer.ok) {
            setViewingFile({ path: full, text: '', mode: 'pdf', pdf: { pages: answer.pages, pictures: answer.pictures, folder } })
            return
          }
          setViewingFile(undefined)
          setViewerRefusal(answer.message)
        })
        .catch(() => setViewerRefusal('Locust could not read that PDF. Nothing was changed.'))
      return
    }
    if (imageMediaType(full) !== undefined) {
      void bridge
        .readWorkspaceImage(full, imageFolder)
        .then((answer) => {
          if (answer.ok) {
            setViewingFile({ path: full, text: answer.dataUrl, mode: 'image' })
            return
          }
          setViewingFile(undefined)
          setViewerRefusal(answer.message)
        })
        .catch(() => setViewerRefusal('Locust could not read that image. Nothing was changed.'))
      return
    }
    void bridge
      .readTextFile(full)
      .then((answer) => {
        if (answer.ok) {
          setViewingFile({
            path: full,
            text: answer.text,
            mode: answer.mode,
            ...(answer.workbook === undefined ? {} : { workbook: answer.workbook }),
            ...(answer.document === undefined ? {} : { document: answer.document })
          })
          // A web page opens WORKING (0.425, Colin: "full functionality,
          // sacrifice nothing"); its source is one tab away.
          if (/\.html?$/i.test(full)) {
            void bridge.pageUrlFor(full).then((page) => {
              if (page.ok) setViewingFile((current) => (current?.path === full ? { ...current, pageUrl: page.url } : current))
            }).catch(() => undefined)
          }
          return
        }
        setViewingFile(undefined)
        setViewerRefusal(answer.message)
      })
      .catch(() => setViewerRefusal('Locust could not read that file. Nothing was changed.'))
  }
  const [screen, setScreen] = useState<Screen>('workroom')
  /*
   * What each teammate has spent this month, from the host, which reads every
   * conversation: the window holds events for the newest twenty, and a total
   * added up here was short by everything older (2026-09-26). Asked for while
   * the Team screen or a teammate's dialog is open, and again as history moves.
   */
  const [spendByTeammate, setSpendByTeammate] = useState<Readonly<Record<string, Spend>>>({})
  const wantsSpend = screen === 'teammates' || editingTeammate !== undefined
  useEffect(() => {
    if (!wantsSpend) return
    let current = true
    void window.desktop?.teammateSpend().then((response) => {
      if (current && response.ok) setSpendByTeammate(response.data.byTeammate)
    }).catch(() => undefined)
    return () => {
      current = false
    }
  }, [wantsSpend, history])
  const [paletteOpen, setPaletteOpen] = useState(false)
  // Accept edits, not Ask. A person who opens a workroom and says "add a
  // discount function" means it; under Ask the sandbox refuses the write and
  // the model pastes its patch into the reply instead, with nothing on screen
  // saying the MODE is why. That was the first run of the app, measured
  // 2026-09-03. Ask stays one click away and a teammate who has run keeps
  // whatever they last ran on.
  const [mode, setMode] = useState<MissionMode>('accept-edits')
  // Runtimes whose last run ended on the account's usage limit, with the
  // runtime's own words, until a run on them completes. Session-only on
  // purpose: the limit is on the provider's clock, and a note that outlived
  // it would be the false claim in the other direction.
  const [limitedRuntimes, setLimitedRuntimes] = useState<ReadonlyMap<string, string>>(new Map())
  /** The latest still-allowed rate-limit reading per runtime, by words. */
  const [usageWindows, setUsageWindows] = useState<ReadonlyMap<string, string>>(new Map())
  /*
   * The readings the catalog asked for (0.390): fresh from the account's own
   * server, so they replace whatever the ledger last kept -- a reading from
   * a run days ago is older than one read a moment ago. A run's live
   * reading, arriving later, replaces them in turn.
   */
  const adoptCatalogUsage = (windows: Readonly<Record<string, string>> | undefined): void => {
    const entries = Object.entries(windows ?? {})
    if (entries.length === 0) return
    setUsageWindows((current) => {
      const next = new Map(current)
      for (const [runtime, said] of entries) next.set(runtime, said)
      return next
    })
  }
  const [route, setRoute] = useState<RouteChoice>({ runtime: 'codex', model: 'account-default' })
  /** Words going back into the chat box (C9): a busy model's message, to send on another. */
  const [handBack, setHandBack] = useState<{ readonly text: string; readonly attachments: readonly string[] }>()
  interface HeldCommand extends ConversationCommand { readonly key: string; readonly carried: boolean }
  const [commandEntries, setCommandEntries] = useState<readonly HeldCommand[]>([])
  const commandEntriesRef = useRef<readonly HeldCommand[]>([])
  commandEntriesRef.current = commandEntries
  const draftKeyRef = useRef('new')
  const commandResults = useRef(new Map<string, OwnCommandResult>())
  const [ownCommandMenu, setOwnCommandMenu] = useState<readonly ListedCommand[]>([])
  const [commandAsk, setCommandAsk] = useState<{ readonly name: string; readonly source: 'global' | 'project'; readonly args: string; readonly lines: readonly string[] }>()
  // What changed about the watched pull request (0.731), shown above the box until dismissed.
  const [pullRequestNewsShown, setPullRequestNewsShown] = useState<PullRequestNews>()
  useEffect(() => window.desktop?.onPullRequestNews((news) => setPullRequestNewsShown(news)), [])
  // A part of a reply the person asked about (SelectionAsk): quoted into the box.
  const [quoteIn, setQuoteIn] = useState<{ readonly quote: string; readonly attachments?: readonly string[] }>()
  // Parts of the browser page waiting in the box. They leave when the conversation changes.
  const [pagePoints, setPagePoints] = useState<readonly PagePointChip[]>([])
  // Intent belongs to the addressed teammate, not to discovery or the last
  // thread visited. Session-only state deliberately clears on app restart.
  const [pickerRoutes, setPickerRoutes] = useState<ReadonlyMap<string, RouteChoice>>(new Map())
  /**
   * Whether the route on screen is the person's own choice.
   *
   * Until they pick one it is a guess, and a guess that names a runtime this
   * machine does not have is the wall the whole first run hits: install
   * exactly what the app told you to, come back, type, press Enter, nothing
   * (QA, 2026-09-06). So an unchosen route follows discovery. A CHOSEN one
   * never moves on its own -- picking Codex deliberately and watching the app
   * change it back would be worse than the original defect.
   */
  const routeChosen = useRef(false)
  /**
   * Installing a runtime from inside the app.
   *
   * One at a time -- the host refuses a second, and the panel disables every
   * other button while one runs -- so a single output line belongs to a
   * single install and nobody loses track of whose error is whose.
   */
  const [installing, setInstalling] = useState<string>()
  const [installLine, setInstallLine] = useState<string>()
  /** Every line npm printed for the install now running, newest last. */
  const [installLog, setInstallLog] = useState<readonly string[]>([])
  const [installFailure, setInstallFailure] = useState<{
    readonly what: string
    readonly next: string
    readonly restart?: boolean
    readonly command?: string
  }>()
  const installStartedAt = useRef(0)
  /**
   * Ask discovery again, from outside the effect that owns it.
   *
   * Main drops its cache and re-probes the moment an install exits cleanly --
   * that is how "npm said fine but there is nothing to run" is detected. The
   * RENDERER was never told, so a successful install left the row saying
   * "Install", the sidebar saying "0 runtimes connected", and the composer
   * still pointed at a runtime that was not there. Pressing the button for
   * the first time is what showed it: `opencode.cmd` on disk, and the app
   * insisting nothing had happened (drive, 2026-09-06).
   */
  const askDiscoveryAgain = useRef<(everything?: boolean) => void>(() => undefined)
  /*
   * Keeping the coding agents current (main/runtime-updates.ts): what it has
   * done, for Settings -- and when an agent has just been updated, the
   * machine is asked again and the models re-read, so what the new version
   * brings (GPT-6-Sol and -Luna, the day this was written) is in the picker
   * without a restart.
   */
  const [runtimeUpdates, setRuntimeUpdates] = useState<RuntimeUpdatesState>()
  const [loginItem, setLoginItem] = useState<LoginItemState>({ openAtLogin: false, available: false })
  const [keepRunning, setKeepRunning] = useState(false)
  const readStartup = (): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    void bridge.readLoginItem().then(setLoginItem).catch(() => undefined)
    void bridge.readKeepRunning().then((state: KeepRunningState) => setKeepRunning(state.keepRunning)).catch(() => undefined)
  }
  useEffect(() => { readStartup() }, [])
  useEffect(() => {
    const bridge = window.desktop
    if (bridge === undefined) return undefined
    void bridge.readRuntimeUpdates().then(setRuntimeUpdates).catch(() => undefined)
    const heard = new Set<string>()
    return bridge.onRuntimeUpdates((state) => {
      setRuntimeUpdates(state)
      const landed = state.agents.flatMap((agent) =>
        agent.status.kind === 'updated' ? [`${agent.runtime}@${agent.status.to}`] : []
      )
      if (landed.some((key) => !heard.has(key))) {
        for (const key of landed) heard.add(key)
        askDiscoveryAgain.current(true)
        readModels()
      }
    })
    // Once: the listener reads the state it is handed.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
  /** Check again starts the count of sweeps over; see the discovery effect. */
  const resetDiscoveryBudget = useRef<() => void>(() => undefined)
  const [installElapsed, setInstallElapsed] = useState(0)

  useEffect(() => {
    const bridge = window.desktop
    if (!bridge) return
    return bridge.onRuntimeInstallProgress(({ line }) => {
      setInstallLine(line)
      // Everything npm said, not just the last thing. The design asks for a
      // disclosure that opens onto the whole transcript, and a person who
      // cannot read an npm trace can still hand the whole thing to someone
      // who can. Bounded so a pathological install cannot grow without end.
      setInstallLog((held) => [...held, line].slice(-INSTALL_LOG_LINES))
    })
  }, [])

  // The elapsed count, which is the honest substitute for a progress bar: npm
  // reports nothing that can become a percentage, so the screen shows the one
  // number it actually has.
  useEffect(() => {
    if (installing === undefined) return
    const tick = window.setInterval(() => {
      setInstallElapsed(Math.max(1, Math.round((Date.now() - installStartedAt.current) / 1000)))
    }, 1000)
    return () => window.clearInterval(tick)
  }, [installing])

  /*
   * "checking for npm", not "starting npm".
   *
   * Before an install runs, the installer re-asks which npm
   * to use. That is right -- the one thing that changes the
   * answer is the person having installed Node since the
   * first screen decided. But on a machine whose npm hangs
   * that probe takes five seconds, and the line claimed npm
   * was starting while nothing of the sort was happening
   * (Fable, pass 2, finding 4: "2s · starting npm… 6s ·
   * starting npm…", then the bundled path). Once npm really
   * does speak, `installLine` carries its own output and
   * this is gone. Home and Settings show the same line (0.780).
   */
  const installLineShown = installing === undefined ? undefined : `${String(installElapsed)}s · ${installLine ?? 'checking for npm…'}`
  const installRuntime = (runtime: string): void => {
    const bridge = window.desktop
    if (!bridge || installing !== undefined) return
    setInstallFailure(undefined)
    setInstallLine(undefined)
    setInstalling(runtime)
    installStartedAt.current = Date.now()
    setInstallElapsed(0)
    void bridge
      .installRuntime(runtime)
      .then((response) => {
        setInstalling(undefined)
        if (response.ok) {
          // Nothing announces success -- the row simply becomes the ready
          // state. But it has to be ASKED: main re-probes on its side, and
          // without this the renderer waits for a timer or a focus event
          // while the screen says nothing was installed.
          setInstallLine(undefined)
          setInstallLog([])
          askDiscoveryAgain.current()
          return
        }
        setInstallFailure({
          what: response.what,
          next: response.next,
          ...(response.restart === true ? { restart: true } : {}),
          // The failure's own command wins when it has one: a permission
          // failure's remedy is not the command that just failed, and showing
          // that one again is what left a first tester stuck (2026-09-07).
          ...(response.command !== undefined
            ? { command: response.command }
            : installCommand(runtime) === undefined
              ? {}
              : { command: installCommand(runtime)! })
        })
      })
      .catch(() => {
        setInstalling(undefined)
        setInstallFailure({
          what: 'The install could not be started. Nothing was installed or changed.',
          next: 'Run the command shown in Settings from a terminal.'
        })
      })
  }
  // Not only at launch: the focus re-probe is what finds a runtime installed
  // mid-session, and that is exactly when this matters -- the person has just
  // come back from installing it.
  /*
   * FOLLOWS THE START ORDER, not only when the placeholder cannot run (0.307).
   * The route starts as `codex / account-default` -- a placeholder nobody
   * chose -- and this used to replace it only when Codex could NOT run. So
   * on any machine where Codex worked, defaultRoute was never asked: 0.303
   * and the first 0.307 package both started a fresh profile on Codex with
   * Claude Code signed in and ready (drive-start-and-lane), and the old
   * "OpenCode first" had never applied there either. Until the person picks
   * a route, it is whatever defaultRoute says now.
   */
  useEffect(() => {
    if (runtimeState.phase !== 'ready' || routeChosen.current) return
    const next = defaultRoute(runtimes)
    if (next.runtime === route.runtime) return
    setRoute(next)
  }, [runtimeState.phase, runtimes, route.runtime])
  // Keyed on WHICH runtimes are usable, not on the array identity: discovery
  // re-runs every few seconds and hands back a new array each time, and
  // re-reading the catalogue on every sweep would be a request per tick.
  const usableKey = runtimes.filter((runtime) => runtimeIsUsable(runtime)).map((runtime) => runtime.id).join(',')

  useEffect(() => {
    if (runtimeState.phase !== 'ready') return
    readModels()
    // Same trigger as the catalogue: both are gated on which runtimes are
    // actually installed, so both are meaningless until discovery settles and
    // both want re-reading when that set changes.
    window.desktop
      ?.listRuntimeArtifacts()
      .then((found) => setCliArtifacts(found))
      .catch(() => setCliArtifacts([]))
  }, [runtimeState.phase, usableKey])
  const [approvals, setApprovals] = useState<readonly MissionApprovalRequest[]>([])
  // The keyboard's Alt+Shift+Down reads which conversations wait for you from here (conversationSteps.ts).
  const approvalsRef = useRef(approvals)
  approvalsRef.current = approvals
  /**
   * Notes on a teammate's diff (diffNotes.ts, 0.376), by conversation, until
   * they go with a message: moving to another conversation and back keeps
   * them where they were written.
   */
  const [diffNotes, setDiffNotes] = useState<ReadonlyMap<string, readonly DiffNote[]>>(new Map())
  // M31: a card goes with its run -- one whose run ended is not left behind.
  useEffect(() => {
    setApprovals((current) => approvalsOfLiveRuns(current, runs))
  }, [runs])
  const [decidingIds, setDecidingIds] = useState<readonly string[]>([])
  const [listedModels, setModels] = useState<readonly PublicModel[]>([])
  // Less any model OpenCode has retired (0.710, missionView.retiredFreeModels): the picker, Home's free start and
  // Compare's defaults all read this one list, so none of them offers a model that can only fail.
  const retiredModels = useMemo(() => retiredFreeModels(history), [history])
  const models = useMemo(() => listedModels.filter((model) => !retiredModels.has(model.id)), [listedModels, retiredModels])
  /*
   * An unchosen route lands on a NAMED FREE model once the catalogue says
   * there is one. Sol's beta finding 1, the only one of the nine that can
   * spend money: after installing the runtime we recommend BECAUSE it needs
   * no account, the composer read `OpenCode / Account Default` -- an unnamed
   * route behind the first Enter a new person presses.
   *
   * Gated on `routeChosen` exactly like the runtime half above it: a route
   * the person picked never moves on its own, and pinning Account Default
   * deliberately has to stay possible. This only ever fires on a profile that
   * has not picked yet, which is the profile the finding is about.
   */
  // The free models that finished a run here, newest first: preferred over the catalogue's first (0.517).
  const freeAnswered = useMemo(() => freeModelsThatAnswered(history), [history])
  // And the free models that are down right now, passed over wherever Locust suggests one (0.711).
  const freeDown = useMemo(() => downFreeModels(history, Date.now()), [history])
  useEffect(() => {
    if (runtimeState.phase !== 'ready' || routeChosen.current) return
    if (route.model !== ACCOUNT_DEFAULT_MODEL) return
    const free = freeStartModel(route.runtime, models, freeAnswered, freeDown)
    if (free === undefined) return
    setRoute((current) =>
      current.model === ACCOUNT_DEFAULT_MODEL ? { ...current, model: free } : current
    )
  }, [runtimeState.phase, route.runtime, route.model, models, freeAnswered, freeDown])
  // What the CLIs already have set up. Read once discovery has settled: the
  // list is gated on which runtimes are installed, so asking earlier would
  // report an empty machine.
  const [cliArtifacts, setCliArtifacts] = useState<readonly PublicRuntimeArtifact[]>([])
  /**
   * Re-read the model catalogue when the runtimes change, and when the picker
   * is opened.
   *
   * It used to be read ONCE, at mount, and never again. The catalogue is
   * built from discovery -- `claudeModelsFrom` returns nothing at all unless
   * Claude's readiness is already `ready` -- and readiness is a probe that
   * finishes whenever it finishes. So if that single read landed before the
   * probe did, the picker offered Claude one row, `account-default`, for the
   * rest of the session, and only a relaunch fixed it. Colin, 2026-09-06:
   * "the claude models aren't showing up anymore, it just says claude account
   * default." Intermittent by timing, which is why it reads as "anymore".
   *
   * The same shape hid OpenCode's models from anyone who installed it
   * mid-session (QA, 2026-09-06): eleven models on the machine, one row in
   * the picker.
   *
   * Main already re-sweeps discovery every ten seconds and serves the
   * catalogue from that sweep, so this asks a question that is cheap and
   * already answered.
   */
  const readModels = (): void => {
    const bridge = window.desktop
    if (!bridge) return
    void bridge
      .listModels()
      .then((response) => {
        if (!response.ok) return
        rememberOwnModels(response.data.models)
        setModels(response.data.models)
        adoptCatalogUsage(response.data.usageWindows)
      })
      .catch(() => {
        // Optional, as it always was: without it the picker offers the
        // account default, which is what a run is launched with anyway.
      })
  }
  /**
   * Ask again while a usable runtime still has no models.
   *
   * Discovery calls a runtime READY as soon as it is installed and signed in,
   * which happens well before its `--list-models` probe answers. So the first
   * catalogue read can legitimately come back with nothing for it -- and
   * `usableKey` above never changes afterwards, because the runtime was
   * already usable, so nothing ever asks again. The picker then offers only
   * `account-default` for that runtime until the app is restarted.
   *
   * Colin, 2026-09-08: "cursor agent is only showing account default now on my
   * end" ... "nvm it fixed it on reset". The same shape was diagnosed for
   * Claude Code's aliases in the 0.36.5 QA pass and never fixed, because the
   * fix looked like re-reading on a timer and nobody wanted a poll.
   *
   * This is a poll, but a self-limiting one: it stops the moment every usable
   * runtime has at least one model, and it gives up after a handful of tries
   * so a runtime that will never report a catalogue (Antigravity has none)
   * cannot keep it running.
   */
  const awaitingModels = runtimes.some(
    (runtime) =>
      runtimeIsUsable(runtime) &&
      isMissionRuntime(runtime.id) &&
      !models.some((model) => model.runtime === runtime.id)
  )
  const modelRetries = useRef(0)
  useEffect(() => {
    if (runtimeState.phase !== 'ready') return
    // A minute in all (0.552): a slow launch's list can take longer than the
    // first few seconds, and the host no longer holds a list missing one.
    if (!awaitingModels || modelRetries.current >= 12) return
    const again = window.setTimeout(() => {
      modelRetries.current += 1
      readModels()
    }, 5_000)
    return () => window.clearTimeout(again)
  }, [awaitingModels, runtimeState.phase, models])
  const [effort, setEffort] = useState<string>()
  /** Each chat's mode and effort, by the same key as its model pick (0.552). */
  const chatSetups = useRef(new Map<string, { readonly mode: MissionMode; readonly effort: string | undefined; readonly kind?: { readonly compare: boolean; readonly blind: boolean; readonly cloud: boolean; readonly computer?: boolean } }>())
  /** What a new chat with nobody starts on, as saved (0.552). */
  const [newChatSetup, setNewChatSetup] = useState<{ readonly mode: MissionMode; readonly effort: string | undefined }>()
  const [swarm, setSwarm] = useState(false)
  /*
   * How many times the person has turned swarm on this session. The title
   * screen flies the swarm across for each new one (HomeCover's SwarmRun) --
   * a count, not the setting, because the setting also turns on by itself
   * at launch, read back from disk, and that is not a moment.
   */
  const [swarmCalls, setSwarmCalls] = useState(0)
  // Off until the workspace says otherwise, and re-read from the host rather
  // than remembered: this is the switch that decides whether a mode which can
  // write anywhere on the machine is offered at all.
  const [autoMode, setAutoMode] = useState(false)
  /** Ask before every connector call. See WorkspaceSettings.askConnectors. */
  const [askConnectors, setAskConnectors] = useState(false)
  /** Ask teammates that can to keep a todo list. See WorkspaceSettings.keepATodoList. */
  const [keepATodoList, setKeepATodoList] = useState(false)
  /** Lend the person's own skills to Claude Code teammates. See WorkspaceSettings.claudeOwnSkills. */
  const [claudeOwnSkills, setClaudeOwnSkills] = useState(false)
  /** Who carries the work on when a plan runs out (0.774). See WorkspaceSettings.planRunsOut. */
  const [planRunsOut, setPlanRunsOut] = useState<PlanRunsOut | undefined>(undefined)
  /** The stopped turn whose Carry on was pressed, until the new turn starts or the press is refused (0.774). */
  const [carryingOn, setCarryingOn] = useState<{ readonly missionId: string; readonly refused?: string } | undefined>(undefined)

  // Switching Auto off takes it away from a window that was sitting on it,
  // rather than leaving a choice the host would refuse at the next send.
  /*
   * Only once the answer is KNOWN. Before the settings have been read,
   * `autoMode` is its initial false, and a conversation reopened at
   * startup on Auto was being knocked down to Accept edits in the gap --
   * the other half of "the next time it will be accept edits".
   */
  const [autoModeKnown, setAutoModeKnown] = useState(false)
  useEffect(() => {
    if (autoModeKnown && !autoMode && mode === 'auto') setMode('accept-edits')
  }, [autoModeKnown, autoMode, mode])
  useEffect(() => {
    if (build?.organizationPolicy === undefined || policyModeAllowed(build.organizationPolicy, mode)) return
    const narrowed = [...POLICY_MODE_ORDER].reverse().find((value) => policyModeAllowed(build.organizationPolicy, value) && modeRunsOn(value, route.runtime, build.platform))
    setMode(narrowed ?? 'ask')
  }, [build?.organizationPolicy, build?.platform, mode, route.runtime])
  const [relay, setRelay] = useState(true)
  /** The autonomy budget: automatic replies one exchange may use. */
  const [relayHopCap, setRelayHopCap] = useState(DEFAULT_RELAY_HOP_CAP)
  /** Whether a teammate may stop another's run to be heard now. Off until asked for. */
  const [interrupt, setInterrupt] = useState(false)
  const [stoppingExchange, setStoppingExchange] = useState(false)
  // Which folder this window works in. Every mission runs here; a person
  // with two projects open needs the title to say which is which.
  const [workspaceName, setWorkspaceName] = useState('Local workspace')
  /** The folder itself, so activity rows can show paths the way a person writes them. */
  const [workspacePath, setWorkspacePath] = useState<string | undefined>(undefined)
  const [cloudEnvironment, setCloudEnvironment] = useState({ folder: '', value: '', edited: false })
  useEffect(() => {
    if (workspacePath === undefined) return
    const folder = workspacePath
    let active = true
    setCloudEnvironment({ folder, value: '', edited: false })
    void window.desktop?.getClaudeCloudEnvironment().then((value) => {
      if (active) setCloudEnvironment((current) => current.folder === folder && !current.edited ? { folder, value: value ?? '', edited: false } : current)
    }).catch(() => undefined)
    return () => { active = false }
  }, [workspacePath])
  // Whether a comparison here may change files (see compareChangesRefusal above).
  useEffect(() => {
    if (!compareOn) return
    let current = true
    void window.desktop?.compareWorksInPlace?.().then((inPlace) => {
      if (current) setCompareInPlace(inPlace === true)
    }).catch(() => undefined)
    void window.desktop?.compareChangesRefusal().then((refusal) => {
      if (!current) return
      setCompareChangesRefusal(refusal)
      if (refusal !== undefined) setCompareChanges(false)
    }).catch(() => undefined)
    return () => {
      current = false
    }
    // And again for another folder (0.553): Colin, 10/02, in a new folder of
    // one file, "still getting this issue on compare mode" -- the answer was
    // the last folder's (his `.claude`), read once when Compare was switched on.
  }, [compareOn, workspacePath])
  // Only the ledger-failure card reads this: it offers to open the folder.
  const [ledgerPath, setLedgerPath] = useState<string | undefined>(undefined)
  const [workspaceMade, setWorkspaceMade] = useState(false)
  // A folder like home or .claude (0.790): the folder chip names why, in amber.
  const [workspaceCaution, setWorkspaceCaution] = useState<string | undefined>(undefined)
  // And the question over the box before its first conversation there, on the view it was asked from.
  const [folderQuestion, setFolderQuestion] = useState<{ readonly why: string; readonly key: string | undefined } | undefined>(undefined)
  // A word about the folder, at shell level: a refused start, a refused
  // choice, or the reopen that follows a successful one.
  const [workspaceNotice, setWorkspaceNotice] = useState<string>()
  /** The folder id the host reports with history, so the sidebar can keep to it. */
  const [workspaceId, setWorkspaceId] = useState<string | undefined>(undefined)
  // Cloud tasks (0.503): this folder's, read when the folder opens.
  useEffect(() => {
    const bridge = window.desktop
    if (bridge === undefined) return
    void bridge.listCloudTasks().then(setCloudTasks).catch(() => undefined)
    void Promise.resolve().then(() => bridge.backgroundRuns()).then(setBackgroundRuns).catch(() => undefined)
    // Claude's sessions too (0.556), so the header can offer the panel after a restart.
    void Promise.resolve().then(() => bridge.listClaudeCloud()).then(setClaudeCloud).catch(() => undefined)
    void bridge.cloudWhere().then(setCloudWhere).catch(() => undefined)
  }, [workspaceId])
  // The folders to offer when this one is not on GitHub, asked only when the panel would show them.
  useEffect(() => {
    // Claude's cloud asks too when the folder is not on GitHub (0.545); its environment is Codex's concern.
    if (!cloudPanel || cloudWhere === undefined || (cloudWhere.repo !== undefined && cloudWhere.environment !== 'missing')) return
    setCloudFolders(undefined)
    void window.desktop?.cloudFolders().then(setCloudFolders).catch(() => setCloudFolders([]))
  }, [cloudPanel, cloudWhere])
  // And followed while one is in the cloud and the panel is open: Codex is asked every 15 s.
  const cloudPending = cloudTasks.some((task) => task.status.state === 'pending')
  useEffect(() => {
    if (!cloudPanel || !cloudPending) return
    const timer = setInterval(() => {
      const bridge = window.desktop
      if (bridge === undefined) return
      for (const task of cloudTasks.filter((entry) => entry.status.state === 'pending')) {
        void bridge.refreshCloudTask(task.taskId).then((next) => {
          if (next !== undefined) setCloudTasks((current) => current.map((entry) => (entry.taskId === next.taskId ? next : entry)))
        }).catch(() => undefined)
      }
    }, 15_000)
    return () => clearInterval(timer)
  }, [cloudPanel, cloudPending, cloudTasks])
  // Faces that just finished or just heard something: a hop and a glance, each
  // for a moment, then still. Keyed by teammate; cleared by their own timers.
  const [recentlyDone, setRecentlyDone] = useState<readonly string[]>([])
  const [recentlyReceived, setRecentlyReceived] = useState<readonly string[]>([])
  // And a message just handed from one to another: the two look at each
  // other where both faces are in one line (glances.ts), for the same moment.
  const [handoffs, setHandoffs] = useState<readonly Handoff[]>([])
  const missionOwnersRef = useRef<Readonly<Record<string, string>>>({})
  const groupsRef = useRef<readonly PublicGroup[]>([])
  const groupMembersRef = useRef<Readonly<Record<string, GroupMembership>>>({})
  /**
   * The id a group membership is keyed by: the conversation, not the turn.
   *
   * The same key `renameMission` uses. A sidebar row stands for an exchange,
   * and filing one reply of it somewhere else would be a group that lies
   * about what it holds.
   */
  /** What group this conversation is in, under any id it has worn. */
  const groupOfConversation = (missionId: string): GroupMembership | undefined => {
    const row = sidebarMissionsRef.current.find((entry) =>
      (entry.memberIds ?? [entry.missionId]).includes(missionId)
    )
    return row === undefined ? groupMembersRef.current[missionId] : heldFor(row, groupMembersRef.current)
  }

  const conversationKeyOf = (missionId: string): string => {
    const row = sidebarMissionsRef.current.find((entry) =>
      (entry.memberIds ?? [entry.missionId]).includes(missionId)
    )
    return row?.rootId ?? row?.missionId ?? missionId
  }
  // Read inside the update listener, which is bound once.
  const historyByIdRef = useRef<ReadonlyMap<string, PublicRecoveredMission>>(new Map())
  const [teammateError, setTeammateError] = useState<string>()
  const [handingOff, setHandingOff] = useState(false)
  /**
   * Who the composer is talking to. A mission is started by messaging a
   * teammate, so this decides who the next mission belongs to, whose waiting
   * workroom messages it is shown, and under whose name it may share.
   */
  const [selectedTeammateId, setSelectedTeammateId] = useState<string>()
  /*
   * A finish the person has not looked at yet (0.380): the Team board's
   * "Just finished". Set when a teammate's run ends while their conversation
   * is not the one on screen; cleared the moment it is.
   */
  const [finishedUnseen, setFinishedUnseen] = useState<ReadonlySet<string>>(new Set())
  // The same, by conversation, for the board's Ready to look at (0.585): turns whose run ended while the person was elsewhere.
  const [finishedUnseenMissions, setFinishedUnseenMissions] = useState<ReadonlySet<string>>(new Set())
  /*
   * SINCE YOU WERE AWAY (0.590, PRD R17): the moment the person was last at
   * the window, from the main process (main/away.ts) -- at start, and again
   * when the window comes back after twenty minutes. Undefined: nothing to say.
   */
  const [awaySince, setAwaySince] = useState<string | undefined>(undefined)
  useEffect(() => {
    void window.desktop?.getAwaySince?.().then((answer) => {
      if (typeof answer?.since === 'string') setAwaySince(answer.since)
    }).catch(() => undefined)
    const off = window.desktop?.onAwaySince?.((since) => setAwaySince(since))
    return () => off?.()
  }, [])
  const lookingAtRef = useRef<{ readonly screen: Screen; readonly teammateId: string | undefined; readonly missionId?: string }>({ screen: 'workroom', teammateId: undefined })
  /**
   * Stops pressed before the run had a name, by the key it had at the time.
   *
   * A run is `starting` from the moment the person presses send until the
   * host answers with its run id, and the box draws the stop button for the
   * whole of it -- correctly, because the runtime is already going. But there
   * was nothing to cancel BY: `cancelMission` reads the run id off the shown
   * run, found none, and returned without a word.
   *
   * MEASURED 2026-09-10, on the second turn of a conversation: `shownKey`
   * was still `pending:2` six seconds after send, so the press did nothing at
   * all and the mission ran to completion -- 200 of 200 numbers, on both
   * Claude Code and Codex, which is how it was told apart from the transport
   * change that day. A first turn resolves fast enough to hide it.
   *
   * A ref rather than state: it is read inside the async start that is
   * already in flight, where a re-render would not reach it.
   */
  const cancelWhenNamedRef = useRef(new Set<string>())
  const pendingUpdatesRef = useRef(new Map<string, CodexMissionUpdate[]>())
  // Clear only after commit: React may evaluate an updater twice, and each
  // evaluation must replay the same queued terminal receipt.
  useEffect(() => {
    for (const runId of runs.keys()) pendingUpdatesRef.current.delete(runId)
  }, [runs])
  const pendingKeyCounter = useRef(0)

  const liveRun = shownKey === undefined ? undefined : runs.get(shownKey)
  const [githubReady, setGithubReady] = useState(false)
  const [issuePicker, setIssuePicker] = useState<readonly ListedIssue[]>()
  const [issueEmpty, setIssueEmpty] = useState<string>()
  const [shownIssue, setShownIssue] = useState<StartedIssue>()
  const pendingIssueRef = useRef<StartedIssue | undefined>(undefined)
  useEffect(() => {
    const bridge = window.desktop?.githubIssues
    if (bridge === undefined || workspacePath === undefined) {
      setGithubReady(false)
      setIssuePicker(undefined)
      return
    }
    let gone = false
    void bridge.ready().then((yes) => {
      if (gone) return
      setGithubReady(yes === true)
      if (yes !== true) setIssuePicker(undefined)
    })
    return () => { gone = true }
  }, [workspacePath])
  const issueMissionId = liveRun?.data?.missionId ?? liveRun?.restoredMission?.missionId
  useEffect(() => {
    if (issueMissionId === undefined) { setShownIssue(undefined); return }
    let gone = false
    void window.desktop?.githubIssues?.remembered(issueMissionId).then((issue) => { if (!gone) setShownIssue(issue) })
    return () => { gone = true }
  }, [issueMissionId])
  const openIssueList = (): void => {
    setIssueEmpty(undefined)
    void window.desktop?.githubIssues?.list().then((issues) => {
      setIssuePicker(issues)
    }).catch(() => {
      setIssuePicker([])
      setIssueEmpty('The issues could not be read. The box is unchanged.')
    })
  }
  const chooseIssue = (number: number): void => {
    void window.desktop?.githubIssues?.open(number).then(async (opened) => {
      if (opened === undefined) {
        setIssueEmpty('That issue could not be read. The box is unchanged.')
        return
      }
      const paths: string[] = []
      for (const image of opened.images) {
        const raw = Uint8Array.from(atob(image.base64), (char) => char.charCodeAt(0))
        const attached = await window.desktop?.attachPasted(image.name, raw).catch(() => undefined)
        if (attached?.ok === true) paths.push(...attached.paths)
      }
      pendingIssueRef.current = opened.issue
      setHandBack({ text: opened.brief, attachments: paths })
      setIssuePicker(undefined)
    }).catch(() => setIssueEmpty('That issue could not be read. The box is unchanged.'))
  }
  /**
   * The shown run as of the LAST render, for handlers that run after an
   * await. A handoff is decided inside an async callback, and reading state
   * from that callback's closure can hand it the value from whichever render
   * created the callback -- often the render before the mission had a runId.
   */
  const liveRunRef = useRef<LiveRunState | undefined>(undefined)
  liveRunRef.current = liveRun
  const runsRef = useRef<ReadonlyMap<string, LiveRunState>>(runs)
  runsRef.current = runs

  /**
   * Re-read what the local history costs.
   *
   * Deleting a mission changes it, and so does running one -- but this was
   * called only on a delete, so Settings kept its launch reading: two
   * missions in, and with both listed in the sidebar and two files on disk,
   * it still said `0 missions · 0 B` (QA pass, 2026-09-05). Stale display,
   * not lost data, and the cheapest honest fix is to read it again whenever
   * the screen that shows it is opened.
   */
  const refreshWorktrees = (): void => {
    void window.desktop
      ?.listWorktrees()
      .then((response) => {
        if (response.ok) setWorktrees({ list: response.data.worktrees, reason: response.data.reason })
      })
      .catch(() => undefined)
  }
  // C1: the host's answer comes back whole -- a copy with uncommitted changes
  // names them, and the row asks before sending them back as the agreement.
  const removeWorktree = async (teammateId: string, discard?: readonly string[]): Promise<WorktreeRemoval | undefined> => {
    const bridge = window.desktop
    if (bridge === undefined) return { message: 'Worktrees are not available here.' }
    try {
      const response = await bridge.removeWorktree(teammateId, discard)
      if (!response.ok) {
        return response.error.code === 'WORKTREE_HAS_CHANGES'
          ? { message: response.error.message, changes: response.error.changes }
          : { message: response.error.message }
      }
      setWorktrees({ list: response.data.worktrees, reason: response.data.reason })
      return undefined
    } catch {
      return { message: 'That worktree could not be removed. It is still on disk with its files intact.' }
    }
  }

  const refreshRuntimeSetup = (): void => {
    void window.desktop
      ?.readRuntimeSetup()
      .then((response) => {
        if (!response.ok) return
        setRuntimeSetup(response.data.runtimes)
        setWorkspaceBrief(response.data.workspaceBrief)
      })
      .catch(() => undefined)
  }

  const refreshStorage = (): void => {
    const bridge = window.desktop
    if (!bridge) return
    void bridge
      .readStorageReport()
      .then((response) => {
        if (response.ok) setStorage(response.data)
      })
      .catch(() => undefined)
  }

  /**
   * What the ledger knows about the accounts, before any live event arrives:
   * a runtime that was at its limit when the window closed is still at its
   * limit when it opens (0.21.2 QA, P2 -- a reload turned AT LIMIT back into
   * READY with no successful run in between). Every read of history seeds
   * from it, the boot read included; the first version of this seeded only
   * the refresh path and the live check caught it.
   */
  const seedLimitsFrom = (response: Awaited<ReturnType<NonNullable<typeof window.desktop>['getMissionHistory']>>): void => {
    if (!response.ok) return
    const windows = Object.entries(response.data.usageWindows ?? {})
    if (windows.length > 0) setUsageWindows((current) => { const next = new Map(current); for (const [runtime, said] of windows) if (!next.has(runtime)) next.set(runtime, said); return next })
    const remembered = Object.entries(response.data.limitedRuntimes)
    if (remembered.length === 0) return
    setLimitedRuntimes((current) => {
      const next = new Map(current)
      for (const [runtime, said] of remembered) if (!next.has(runtime)) next.set(runtime, said)
      return next
    })
  }

  /** The history read again; resolves once it is applied (W10: a turn that came back is opened after, not before). */
  const refreshHistoryNow = async (): Promise<void> => {
    const bridge = window.desktop
    if (!bridge) return
    await bridge
      .getMissionHistory(heldDigests(historyRef.current))
      .then((response) => {
        seedLimitsFrom(response)
        applyHistory(response)
      })
      .catch(() => undefined)
  }
  const refreshHistory = (): void => {
    void refreshHistoryNow()
  }

  useEffect(() => {
    const wanted = openAfterImport.current
    if (wanted === undefined || !history.some((mission) => mission.missionId === wanted)) return
    openAfterImport.current = undefined
    openMission(wanted)
  }, [history])

  /**
   * One commit for every update held in a frame, in the order they came.
   * See streamFrames.ts, and the note where updates are pushed into it.
   */
  const frameBatcher = useRef(
    createFrameBatcher<Extract<CodexMissionUpdate, { readonly runId: string }>>((updates) => {
      setRuns((current) => {
        // ONE copy of the map per frame, made at the first update that changes
        // a run, not one per update (Sol's optimization check, 2026-10-02):
        // a burst of fragments copied the whole map once each. Same order,
        // same result; `current` itself is never touched.
        let next: Map<string, LiveRunState> | undefined
        for (const update of updates) {
          const run = (next ?? current).get(update.runId)
          if (run !== undefined) {
            next ??= new Map(current)
            next.set(update.runId, applyMissionUpdate(run, update))
            continue
          }
          // A run restored by history after a reload can send its next update
          // after the initial read. Adopt it now, not only the newest run.
          const recorded = historyRef.current.find((mission) => mission.runId === update.runId)
          if (recorded !== undefined) {
            const queued = pendingUpdatesRef.current.get(update.runId) ?? []
            next ??= new Map(current)
            next.set(update.runId, [...queued, update].reduce(applyMissionUpdate, reopenedRun(recorded, historyByIdRef.current)))
            continue
          }
          // A run whose start receipt has not come back yet: hold its updates
          // until the receipt names its runId, then replay them in order.
          // A side effect inside an updater, which React may run twice (it
          // does under StrictMode in a dev build): the same update is held
          // once, not replayed twice (a B4 lead).
          const queued = pendingUpdatesRef.current.get(update.runId) ?? []
          if (!queued.includes(update)) pendingUpdatesRef.current.set(update.runId, [...queued, update].slice(-500))
        }
        return next ?? current
      })
    })
  )

  useEffect(() => {
    let active = true
    const bridge = window.desktop
    if (!bridge) {
      setRuntimeState({ phase: 'error' })
      return () => {
        active = false
      }
    }

    const removeApprovalListener = bridge.onMissionApproval((request) => {
      // Append rather than replace: the runtime can have more than one action
      // waiting, and dropping an earlier one would strand its turn.
      setApprovals((current) =>
        current.some((entry) => entry.approvalId === request.approvalId) ? current : [...current, request]
      )
    })
    // A question answered somewhere else -- in Antigravity's own window --
    // or whose run ended leaves the card with nothing to ask.
    const removeWithdrawnListener = bridge.onMissionApprovalWithdrawn((approvalId) => {
      setApprovals((current) => current.filter((entry) => entry.approvalId !== approvalId))
    })

    const removeMissionListener = bridge.onCodexMissionUpdate((update) => {
      // A start's phase (0.602) arrives before the run knows its id: matched by teammate, not by id.
      if (update.kind === 'start-phase') setRuns((current) => withStartPhase(current, update))
      if (update.kind === 'cloud-turn') setCloudTurns((current) => new Map(current).set(update.turn.missionId, update.turn))
      if (update.kind === 'cloud-setup') {
        setCloudSetup((current) => {
          if (current?.phase !== 'working') return current
          const { waiting: _was, ...rest } = current
          return { ...rest, step: update.step, ...(update.waiting === true ? { waiting: true as const } : {}) }
        })
      }
      if (update.kind === 'cloud-sign-in') {
        setCloudSetup((current) =>
          current?.phase === 'signing-in' && current.computer === update.name && current.agent === update.agent
            ? { ...current, step: update.step, ...(update.step === 'waking' ? { woke: true as const } : {}) }
            : current)
      }
      // Your own benchmark (0.763): the comparisons its runs again changed, and where each batch is.
      if (update.kind === 'compare-reruns') {
        if (update.compares.length > 0) setCompares((current) => [...current.filter((one) => !update.compares.some((next) => next.compareId === one.compareId)), ...update.compares])
        setRerunProgress(update.progress)
      }
      // A3.3: kept by mission, apart from the run state -- a finished run is
      // rebuilt from its record, which holds no check, and a result that
      // landed first was wiped by that rebuild (drive, 2026-09-25).
      if (update.kind === 'edit-check') {
        const { kind: _kind, runId: _runId, missionId, ...shown } = update
        setEditChecks((current) => new Map(current).set(missionId, shown))
      }
      // A finished mission hops once; a message that just arrived earns a
      // glance. Both are moments, so both clear themselves.
      if (update.kind === 'event') {
        const agent = update.event.sourceAdapter
        const runtime = accountUsageKey(agent, accountFromRoute(update.event.resolvedRouteId ?? ''))
        if (update.event.type === 'adapter.diagnostic' && /\.usage_window$/.test(update.event.payload.code)) {
          const said = `${update.event.payload.message} · from a run at ${update.event.occurredAt}`
          setUsageWindows((current) => (current.get(runtime) === said ? current : new Map(current).set(runtime, said)))
        }
        if (update.event.type === 'route.limit_detected' && update.event.payload.kind === 'quota-exhausted') {
          const said = update.event.payload.message
          setLimitedRuntimes((current) => (current.get(runtime) === said ? current : new Map(current).set(runtime, said)))
        }
        if (update.event.type === 'run.completed') {
          // W12: an ACP run (Copilot) told Locust what it can do as it started; the row in Settings reads it now.
          if (agent === 'copilot') askDiscoveryAgain.current()
          setLimitedRuntimes((current) => {
            if (!current.has(runtime)) return current
            const next = new Map(current)
            next.delete(runtime)
            return next
          })
        }
      }
      if (update.kind === 'event' && (update.event.type === 'run.completed' || update.event.type === 'run.failed')) {
        const owner = missionOwnersRef.current[update.missionId]
        const looking = lookingAtRef.current
        if (owner !== undefined && !(looking.screen === 'workroom' && looking.teammateId === owner)) {
          setFinishedUnseen((current) => (current.has(owner) ? current : new Set([...current, owner])))
        }
        // By conversation too (0.585): a finish the person is not looking at waits on the board.
        if (!(looking.screen === 'workroom' && looking.missionId === update.missionId)) {
          const ended = update.missionId
          setFinishedUnseenMissions((current) => (current.has(ended) ? current : new Set([...current, ended])))
        }
      }
      if (update.kind === 'event' && update.event.type === 'run.completed') {
        const owner = missionOwnersRef.current[update.missionId]
        if (owner !== undefined) {
          setRecentlyDone((current) => [...current.filter((id) => id !== owner), owner])
          setTimeout(() => setRecentlyDone((current) => current.filter((id) => id !== owner)), DONE_HOP_MS)
        }
      }
      if (update.kind === 'peer-message' && update.message.direction === 'posted') {
        const to = update.message.to.teammateId
        setRecentlyReceived((current) => [...current.filter((id) => id !== to), to])
        setTimeout(() => setRecentlyReceived((current) => current.filter((id) => id !== to)), RECEIVED_GLANCE_MS)
        const handoff: Handoff = { key: update.message.messageId, from: update.message.from.teammateId, to }
        setHandoffs((current) => [...current.filter((held) => held.key !== handoff.key), handoff])
        setTimeout(() => setHandoffs((current) => current.filter((held) => held.key !== handoff.key)), RECEIVED_GLANCE_MS)
      }
      if (update.kind === 'room-posted') {
        refreshRooms()
        return
      }
      if (update.kind === 'room-changed') {
        /*
         * A teammate's reply moved a board. Re-read the rooms so the screen
         * shows the board as the host now holds it -- and say NOTHING extra.
         *
         * This used to narrate the move under the composer: "Jimothy took on
         * X. Jimothy finished X. Jimothy handed X to Yurt." -- directly below
         * a board already showing X as IN HAND beside Jimothy's face. One
         * fact in two registers, which is the thing this app refuses
         * everywhere else. Colin, 2026-09-11: "that texxt under the chat box
         * needs to go away."
         *
         * The notice itself is kept for what a person cannot read off the
         * board: a REFUSAL. Those are set where they happen.
         */
        refreshRooms()
        return
      }
      if (update.kind === 'runtime-commands-changed') {
        rereadRuntimeCommands()
        return
      }
      if (update.kind === 'models-changed') {
        readModels()
        return
      }
      // A background turn moved or came back (W10): the list is read again, and so is the conversation it joined.
      if (update.kind === 'background-changed') {
        void window.desktop?.backgroundRuns().then((runs) => {
          setBackgroundRuns((before) => {
            const back = runs.filter((run) => run.broughtIn !== undefined && !before.some((old) => old.id === run.id && old.broughtIn !== undefined))
            if (back.length > 0) {
              // The conversation on screen is the one it came back into: shown again, with the turn in it --
              // once the history holds that turn, or there is nothing new to open.
              // Whose it is first (catch-up gave it the conversation's teammate), then the history. The conversation
              // on screen follows it by itself once the history holds it (the effect beside openMissionRef).
              void Promise.all([
                window.desktop?.listTeammates().then((roster) => {
                  if (roster.ok) {
                    setMissionOwners(roster.data.missionOwners)
                    rememberRosterNames(roster.data)
                  }
                }).catch(() => undefined),
                refreshHistoryNow()
              ])
            }
            return runs
          })
        }).catch(() => undefined)
        return
      }
      if (update.kind === 'memory-changed') {
        refreshMemories()
        if ((update.aboutYouSuggested ?? []).length > 0) rereadAboutYou()
        setMemoryNotice(memoryChangedNotice(update))
        setMemoryNoticeWaits(noticeWaits(update))
        return
      }
      if (update.kind === 'routine-blocked') {
        // A scheduled routine that would not start. It has no run, so it is
        // its own branch rather than a run's update, and it is kept until a
        // person reads it -- nobody is watching a schedule fire.
        setAutomationNotice(
          `${update.name} did not start: ${update.message} Trying again at ${new Date(update.retryAt).toLocaleTimeString()}.`
        )
        return
      }
      if (update.kind === 'routine-recovery-changed') {
        void reloadRoutines()
        return
      }
      if (update.kind === 'relay-starting') {
        setRelayStarting((current) => ({
          ...current,
          [update.teammateId]: { teammateId: update.teammateId, answering: update.answering }
        }))
        return
      }
      if (update.kind === 'relay-start-settled') {
        setRelayStarting((current) => {
          if (current[update.teammateId] === undefined) return current
          const next = { ...current }
          delete next[update.teammateId]
          return next
        })
        return
      }
      if (update.kind === 'mission-started') {
        // A teammate replying on their own. The host started it; the renderer
        // adopts it exactly as it adopts a run it asked for, so the sidebar
        // shows them working from this moment rather than after a refresh.
        const startedFor = update.teammateId
        if (startedFor !== undefined) setMissionOwners((current) => ({ ...current, [update.missionId]: startedFor }))
        // The hub moved, or began. Mirrored so the face opens it from this
        // moment rather than after the roster is next re-read.
        if (update.hubMissionId !== undefined) {
          const hub = update.hubMissionId
          setTeammates((current) =>
            current.map((teammate) => (teammate.teammateId === update.teammateId ? { ...teammate, hubMissionId: hub } : teammate))
          )
          // And the row's NAME. The host names a hub the first time it is
          // made, but the titles here were read at launch -- so the row wore
          // its root prompt ("Wren asked: ...") until the next full refresh.
          // Measured by `_smoke/hub-smoke.mjs` on 0.234.0 before this line.
          void window.desktop?.listTeammates().then((listed) => {
            if (listed.ok) rememberRosterNames(listed.data)
          }).catch(() => undefined)
        }
        // A routine that started on its own: the Team card's run count and
        // next run moved on disk, and nobody pressed anything to refresh them.
        if (update.startedBy?.kind === 'routine') void reloadRoutines()
        // A panel's run (0.771: a commit message, a walkthrough): hidden from this moment, and once it is in the record.
        if (isPanelRunKind(update.startedBy?.kind)) {
          const asked = update.missionId
          setPanelRunIds((ids) => (ids.includes(asked) ? ids : [...ids, asked]))
        }
        /*
         * A room start, so re-read the rooms NOW.
         *
         * The host writes the post before it asks anyone and announces each
         * run as it begins -- but the room on screen is drawn from the rooms
         * in renderer state, and those only arrived with the post's own
         * response, which lands after every member has been tried. So the
         * host could be three runs in and the room still showed nothing.
         *
         * This is the other half of the fix for the 45 seconds Astra
         * measured between a process existing and its row appearing.
         */
        if (update.startedBy?.kind === 'room') refreshRooms()
        const queued = pendingUpdatesRef.current.get(update.runId) ?? []
        pendingUpdatesRef.current.delete(update.runId)
        // The turn it continues: a reply's, or -- for a reply on another agent, which the host starts as a switch --
        // the turn it switched from (0.774: a turn carried on after a plan ran out is always the second kind).
        const continues = update.data.followsUp?.missionId ?? update.data.switchedFrom?.missionId
        const switched = update.data.switchedFrom
        setRuns((current) => {
          if (current.has(update.runId)) return current
          let next: LiveRunState = {
            prompt: update.prompt,
            data: update.data,
            phase: 'running',
            events: [],
            /*
             * When it started: now, give or take the one message that said so.
             *
             * A run the window asked for is stamped when it asks; one the
             * host started -- a room post's member, a teammate's reply, a
             * routine -- was adopted with no time at all. Its row then had
             * no age and sorted last, and a room's row fell back to the day
             * the room was made: "pair 15h" straight after two posts
             * (drive-room-remembers, 2026-09-26).
             */
            startedAtIso: new Date().toISOString(),
            ...(update.teammateId === undefined ? {} : { teammateId: update.teammateId }),
            peerMessages: update.data.peerMessages,
            startedBy: update.startedBy,
            ...(continues === undefined
              ? {}
              : {
                  earlierTurns: earlierTurnsOf(continues, current, historyByIdRef.current)
                }),
            // The seam, the moment it happens, as for a reply the person sent on another agent.
            ...(switched === undefined
              ? {}
              : {
                  switchedFrom: {
                    from: switched.runtime,
                    to: update.data.runtime,
                    ...(switched.model === undefined ? {} : { fromModel: switched.model }),
                    toModel: update.data.model,
                    at: new Date().toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }),
                    unsettledCount: switched.unsettledCount,
                    omittedBriefing: switched.omittedBriefing,
                    ...(update.startedBy?.kind === 'carry-on' ? { carriedOn: true as const } : {})
                  }
                })
          }
          for (const held of queued) next = applyMissionUpdate(next, held)
          return withNewRun(current, update.runId, next)
        })
        // The answer lands in the thread that asked: if that thread is the
        // one on screen, follow it to its new turn, as a person's own reply
        // would be followed. Another conversation on screen is left alone.
        const shown = liveRunRef.current
        if (continues !== undefined && shown?.data?.missionId === continues) {
          setShownKey(update.runId)
        }
        // A press of Carry on is answered by the turn it started (0.774).
        if (update.startedBy?.kind === 'carry-on') setCarryingOn(undefined)
        // A turn a call with a teammate started (0.747): kept with the call, so its words go around it.
        if (update.startedBy?.kind === 'mcp' && update.startedBy.via === 'voice' && update.teammateId !== undefined && update.teammateId === liveCall()?.teammateId) {
          addLiveCallTurn(update.missionId)
        }
        // And every turn a call started, to anyone, is one of its to-dos (0.755): listed in the strip, summed up after.
        if (update.startedBy?.kind === 'mcp' && update.startedBy.via === 'voice' && update.teammateId !== undefined) {
          const teammateId = update.teammateId
          addCallHandOff({ missionId: update.missionId, teammateId, name: teammatesRef.current.find((entry) => entry.teammateId === teammateId)?.name ?? '', task: update.prompt, at: Date.now() })
        }
        // A call with a teammate who had no conversation on screen (0.746): its first ask starts one, and the window
        // opens it, as a message the person typed would. A hand-off to someone else in the same call stays a row.
        const talking = talkToRef.current
        if (update.startedBy?.kind === 'mcp' && update.startedBy.via === 'voice' && update.data.followsUp === undefined
          && talking !== undefined && talking.conversationId === undefined && update.teammateId === talking.teammateId) {
          setShownKey(update.runId)
        }
        return
      }
      /*
       * Deltas land once per FRAME, not once per token.
       *
       * Legacy single deltas still use the frame queue. The mission host
       * already coalesces message-deltas; apply that batch in one update
       * immediately, without adding another frame to its delivery deadline.
       * Activity flushes anything ahead of it. See streamFrames.ts.
       */
      // Only an update that belongs to a run goes to a run. The kinds without
      // a runId were all handled and returned above; this is the same fact
      // said where the type checker can see it.
      if (!('runId' in update)) return
      const isDelta = update.kind === 'event' && update.event.type === 'message.delta'
      frameBatcher.current.push(update, !isDelta)
    })

    const stopUpdates = bridge.onUpdateState((state) => {
      if (active) setUpdate(state)
    })

    void bridge
      .readStorageReport()
      .then((response) => {
        if (active && response.ok) setStorage(response.data)
      })
      .catch(() => undefined)

    void bridge
      .getAppInfo()
      .then((info) => {
        if (active) {
          setBuild({ version: info.version, packaged: info.packaged, platform: info.platform, organizationPolicy: info.organizationPolicy })
          setWorkspaceName(info.workspaceName)
          setWorkspacePath(info.workspacePath.length === 0 ? undefined : info.workspacePath)
          setWorkspaceMade(info.workspaceMade === true)
          setWorkspaceCaution(info.workspaceCaution)
          setLedgerPath(info.ledgerPath.length === 0 ? undefined : info.ledgerPath)
        }
      })
      .catch(() => undefined)

    // When the host last actually swept. The host answers from a ten-second
    // cache, and two asks inside it are one sweep, not two.
    let lastCheckedAt: string | undefined
    // True until the first answer says otherwise: an answer that never comes
    // is exactly the case to ask again about.
    let unanswered = true
    // The latest answer, so a re-ask can name the runtimes it is for.
    let known: readonly PublicRuntimeStatus[] = []
    /*
     * Which agents' checks have FINISHED, by the host's word (0.639): an
     * answer that still calls one of them "being checked" crossed that
     * finish, and the held answer is read once more (staleChecking).
     */
    const finishedChecks = new Set<string>()
    let rereadSoon: ReturnType<typeof setTimeout> | undefined
    // A Sign in window was opened: the next return to this window asks at
    // once, whatever the gap, because that is when the answer changes.
    let signInOpened = false
    const onSignInOpened = (): void => {
      signInOpened = true
    }
    window.addEventListener(SIGN_IN_OPENED_EVENT, onSignInOpened)
    // A sign-in window closed: ask now, not only when this window next has focus (0.545).
    let askAfterSignIn: (() => void) | undefined
    const stopSignInClosed = bridge.onSignInClosed?.(() => askAfterSignIn?.()) ?? (() => undefined)
    void bridge
      .getLocalRuntimes()
      .then((response) => {
        if (!active) return
        if (response.ok) {
          lastCheckedAt = response.data.checkedAt
          known = response.data.runtimes
          unanswered = worthAskingAgain(response.data.runtimes)
          readAgainIfStale()
        }
        setRuntimeState(
          response.ok
            ? {
                phase: 'ready',
                runtimes: response.data.runtimes,
                npmPresent: response.data.npmPresent,
                npmIsBundled: response.data.npmIsBundled,
                npmDidNotAnswer: response.data.npmDidNotAnswer
              }
            : { phase: 'error' }
        )
      })
      .catch(() => {
        if (active) setRuntimeState({ phase: 'error' })
      })
    // Discovery ran once at launch and never again, so a runtime slow on
    // its first probe stayed UNAVAILABLE for the whole session (Colin,
    // 2026-09-05). Ask again while any runtime is still CHECKING -- three
    // times, fifteen seconds apart, past the host's ten-second cache -- and
    // once more whenever the window comes back into focus, so a sign-in
    // done elsewhere shows without a relaunch.
    /*
     * SWEEPS are counted, not answers. This counted answers, and two of the
     * first four came from the host's ten-second cache -- the first focus
     * event and the first re-check timer both asked inside it -- so the rows
     * said NOT ANSWERING after the SECOND real sweep while the tooltip
     * promised four, and the third sweep ran under a screen that had
     * already given up (Fable, pass 1, finding 6, with the shims' own log).
     * A cached answer carries the same `checkedAt` as the sweep it came
     * from; it reschedules and counts nothing. After the fourth sweep with
     * something still unanswered, no more are asked until Check again,
     * which starts the count over -- a hung CLI was being probed for ten of
     * every twenty-six seconds, for ever.
     */
    const SWEEPS_BEFORE_GIVING_UP = 4
    let sweeps = 1
    let gaveUp = false
    let lastAsked = Date.now()
    // `everything` drops the host's caches too, npm included. Only Check
    // again passes it: the automatic sweeps stay as cheap as they were.
    /*
     * The held answer, read again: no agent named, so the host asks nothing
     * new -- it returns what it holds, which has the late answer in it.
     */
    function readHeld(): void {
      const desktop = window.desktop
      if (desktop === undefined) return
      void desktop
        .getLocalRuntimes()
        .then((response) => {
          if (!active || !response.ok) return
          lastCheckedAt = response.data.checkedAt
          known = response.data.runtimes
          unanswered = worthAskingAgain(response.data.runtimes)
          readAgainIfStale()
          setRuntimeState((held) => ({
            phase: 'ready',
            runtimes: keepWhatWasKnown(held.phase === 'ready' ? held.runtimes : [], response.data.runtimes),
            npmPresent: response.data.npmPresent,
            npmIsBundled: response.data.npmIsBundled,
            npmDidNotAnswer: response.data.npmDidNotAnswer
          }))
        })
        .catch(() => undefined)
    }
    function readAgainIfStale(): void {
      const stale = staleChecking(known, finishedChecks)
      if (stale.length === 0 || rereadSoon !== undefined) return
      // Once per finish: a NEW check of the same agent says so with its own finish.
      for (const id of stale) finishedChecks.delete(id)
      rereadSoon = setTimeout(() => {
        rereadSoon = undefined
        if (active) readHeld()
      }, LATE_ANSWER_REREAD_MS)
    }
    const askAgain = (everything = false): void => {
      if (!active || gaveUp) return
      lastAsked = Date.now()
      const waitingOn = unreadyRuntimes(known)
      void bridge
        .getLocalRuntimes(everything, everything || waitingOn.length === 0 ? undefined : waitingOn)
        .then((response) => {
          if (!active || gaveUp) return
          /*
           * A REFUSED ANSWER IS NOT THE END OF ASKING.
           *
           * This returned on `!response.ok` without rescheduling, so a host
           * that answered `discoveryFailed()` once ended EVERY re-check chain
           * for the session -- the screen kept whatever it had and nothing
           * asked again until the window happened to regain focus (Fable,
           * pass 2, finding 5).
           *
           * A discovery that could not run is the case where asking again
           * matters most, and it costs one more timer. Not counted as a
           * sweep, because the give-up budget is about CLIs that will not
           * answer, and this is the host not having answered at all -- a
           * failure the person cannot repair by pressing Check again four
           * times.
           */
          if (!response.ok) {
            setTimeout(askAgain, RUNTIME_RECHECK_MS)
            return
          }
          const fresh = response.data.checkedAt !== lastCheckedAt
          lastCheckedAt = response.data.checkedAt
          known = response.data.runtimes
          readAgainIfStale()
          // A re-check must not make the screen go backwards: a probe that
          // has not answered yet keeps whatever the last sweep established.
          setRuntimeState((held) => ({
            phase: 'ready',
            runtimes: keepWhatWasKnown(held.phase === 'ready' ? held.runtimes : [], response.data.runtimes),
            npmPresent: response.data.npmPresent,
            npmIsBundled: response.data.npmIsBundled,
            npmDidNotAnswer: response.data.npmDidNotAnswer
          }))
          /*
           * A CHECK STILL RUNNING IS NOT A CLI THAT WILL NOT ANSWER (0.639).
           *
           * Since 0.634 an agent the sweep went on without is `checking`, and
           * its own finish brings the answer (the late watch below). Counted
           * here, it spent this give-up budget: on a busy machine three slow
           * agents answered late one after another, each re-ask still found
           * OpenCode being checked, the window gave up -- and when OpenCode
           * answered twenty seconds in, `askAgain` returned at once and its
           * models said CHECKING for good (measured, packaged 0.638 and 0.639,
           * the CPU loaded; probe-picker-after-a-late-check.mjs).
           */
          const stillChecking = response.data.runtimes.some(
            (entry) => entry.installed && entry.checking !== true && (entry.status === 'probe-failed' || entry.status === 'offline')
          )
          unanswered = worthAskingAgain(response.data.runtimes)
          if (!stillChecking) return
          if (!fresh) {
            setTimeout(askAgain, RUNTIME_RECHECK_MS)
            return
          }
          sweeps += 1
          if (sweeps < SWEEPS_BEFORE_GIVING_UP) {
            setTimeout(askAgain, RUNTIME_RECHECK_MS)
          } else {
            // Asked, asked again, and again: the screen has to stop saying
            // "shortly" and offer a way past a CLI that will never answer.
            gaveUp = true
            setRuntimeState((held) => (held.phase === 'ready' ? { ...held, gaveUp: true } : held))
          }
        })
        // The host never answered at all -- the same case as a refusal,
        // and the same answer: ask again rather than stop for the session.
        .catch(() => {
          if (active && !gaveUp) setTimeout(askAgain, RUNTIME_RECHECK_MS)
        })
    }
    resetDiscoveryBudget.current = () => {
      sweeps = 0
      gaveUp = false
    }
    // Both gated on there being something to learn (Fable's probing review,
    // #2: measured, the 15 s re-check re-swept a machine with nothing to
    // learn, and every return to the window did the same).
    const firstRecheck = setTimeout(() => {
      if (unanswered) askAgain()
    }, RUNTIME_RECHECK_MS)
    askAfterSignIn = () => {
      signInOpened = false
      askAgain()
    }
    const onFocus = (): void => {
      if (signInOpened) {
        signInOpened = false
        askAgain()
        return
      }
      if (unanswered && Date.now() - lastAsked >= RUNTIME_RECHECK_MIN_GAP_MS) askAgain()
    }
    // So an install can ask for a fresh answer the moment it finishes.
    askDiscoveryAgain.current = askAgain
    window.addEventListener('focus', onFocus)
    /*
     * No agent's check holds the sweep up (main/sweep-settle.ts): one that
     * answers after the sweep did showed "checking" until the 15-second
     * re-check. The host says when it lands, and this asks once -- only for a
     * row that is waiting on exactly that answer.
     */
    const stopLateWatch =
      bridge.onDiscoveryEvent?.((event) => {
        if (event.kind !== 'probe.finished') return
        /*
         * Its answer is stored when the host says it finished (a tick after),
         * so read what the host holds: no agent is asked again -- asking
         * started a NEW check of an agent that had just answered -- and the
         * read is not stopped by the give-up budget, which this answer is
         * exactly the end of (0.639).
         */
        finishedChecks.add(event.id)
        readAgainIfStale()
      }) ?? (() => undefined)

    void bridge
      .readWorkspaceSettings()
      .then((settings) => {
        if (active) {
          setSwarm(settings.swarm === true)
          setRelay(settings.relay === true)
          setAutoMode(settings.autoMode === true)
          setAutoModeKnown(true)
          setPlanRunsOut(settings.planRunsOut ?? undefined)
          setAskConnectors(settings.askConnectors === true)
          setKeepATodoList(settings.keepATodoList === true)
          setClaudeOwnSkills(settings.claudeOwnSkills === true)
          setCheckCommand(settings.checkCommand ?? '')
          setAboutYou(settings.aboutYou ?? '')
          setAboutYouSuggestions(settings.aboutYouSuggestions ?? [])
          setRelayHopCap(settings.relayHopCap)
          setInterrupt(settings.interrupt)
          setMemoryMode(settings.memoryMode)
          setLayout(isLayoutPreference(settings.layout) ? settings.layout : 'auto')
          setTube(settings.tube)
          setReplySize(settings.replySize)
          // The model a new chat with nobody starts on (0.552).
          const newChat = settings.newChatRoute
          if (newChat !== undefined) {
            setPickerRoutes((current) => (current.has('new') ? current : new Map(current).set('new', { runtime: newChat.runtime, model: newChat.model, ...(newChat.account === undefined ? {} : { account: newChat.account }) })))
            setNewChatSetup({ mode: newChat.mode, effort: newChat.effort })
          }
      if (settings.metal !== undefined) setMetal(settings.metal)
      if (settings.metalStrength !== undefined) setMetalStrength(settings.metalStrength)
      if (settings.metalMotion !== undefined) setMetalMotion(settings.metalMotion)
      if (settings.metalBend !== undefined) setMetalBend(settings.metalBend)
      setTerminalFacesShown(settings.terminalFaces !== false)
      setPlushShown(settings.plush === true)
        }
      })
      .catch(() => undefined)

    void bridge
      .listModels()
      .then((response) => {
        if (!active || !response.ok) return
        // Before the models land, so the first render names your own ones.
        rememberOwnModels(response.data.models)
        setModels(response.data.models)
        adoptCatalogUsage(response.data.usageWindows)
      })
      .catch(() => {
        // The catalog is optional: without it the picker offers the account
        // default, which is what the process is launched with anyway.
      })

    // What changed in this build, read from the file that shipped with it.
    void bridge
      .getChangelog()
      .then((answer) => { if (active) setChangelog(answer) })
      .catch(() => undefined)
    void bridge
      .listTeammates()
      .then((response) => {
        if (!active) return
        noteStore('teammates', response.ok)
        if (!response.ok) return
        setTeammates(response.data.teammates)
        /*
         * BOTH maps, every time, and this one is why.
         *
         * `missionTitles` shipped in 0.141.0 read by one refresh path and
         * not by this one -- the roster read that runs at startup. So a name
         * you typed was written to disk correctly and never loaded again:
         * it survived until the window reloaded and then the row went back
         * to the first line of the prompt. Colin, 2026-09-15: "i renamed
         * some of my missions and the name didnt save."
         *
         * It saved. It was never read. The drive that passed had checked
         * `listTeammates()` returned the title and had never restarted the
         * app, which is the only place the difference shows.
         */
        setMissionOwners(response.data.missionOwners)
        rememberRosterNames(response.data)
      })
      .catch(() => {
        // The roster is optional at startup; missions still run without it.
      })

    void bridge
      .listGroups()
      .then((response) => {
        if (!active) return
        noteStore('groups', response.ok)
        if (!response.ok) return
        setGroups(response.data.groups)
        setGroupMembers(response.data.members)
        setGroupLeft(response.data.left)
      })
      .catch(() => {
        // Groups are optional at startup; every conversation is simply
        // ungrouped until they are read.
      })

    void bridge
      .listRoutines()
      .then((response) => {
        if (!active) return
        noteStore('routines', response.ok)
        if (!response.ok) return
        setRoutines(response.data.routines)
      })
      .catch(() => {
        // Routines are optional too: without them the app is what it was.
      })

    void bridge
      .listRooms()
      .then((response) => {
        if (!active) return
        noteStore('rooms', response.ok)
        if (!response.ok) return
        setRooms(response.data.rooms)
      })
      .catch(() => {
        // Rooms are optional in the same way.
      })

    void bridge
      .listMemories()
      .then((response) => {
        if (!active) return
        adoptMemories(response)
      })
      .catch(() => {
        // Memory is optional in the same way.
      })

    void bridge
      .readRuntimeSetup()
      .then((response) => {
        if (!active || !response.ok) return
        setRuntimeSetup(response.data.runtimes)
        setWorkspaceBrief(response.data.workspaceBrief)
      })
      .catch(() => {
        // A runtime's own configuration is shown when it can be read, never guessed.
      })

    void bridge
      .listWorktrees()
      .then((response) => {
        if (active && response.ok) setWorktrees({ list: response.data.worktrees, reason: response.data.reason })
      })
      .catch(() => {
        // A runtime's own configuration is shown when it can be read, never guessed.
      })

    void bridge
      .getMissionHistory()
      .then((response) => {
        seedLimitsFrom(response)
        if (!active) return
        // Everything the response decides, in one place -- see `applyHistory`.
        // This handler used to do it inline while the other two reads did not,
        // which is how they came to disagree.
        applyHistory(response)
        if (!response.ok) return
        setWorkspaceId(response.data.currentWorkspaceId)
        // The most recent mission IN THIS FOLDER opens on launch. It used to be
        // the most recent mission anywhere, so opening Locust in a new project
        // greeted you with a conversation from a different one -- measured
        // 2026-09-03. History still lists every mission; what changes is which
        // one this window opens on, and a folder with no missions opens empty.
        const latest = response.data.missions.find(
          (mission) => mission.workspaceId === response.data.currentWorkspaceId
        )
        if (latest === undefined) return
        // Only a run still under way is put on screen at launch. A finished
        // conversation is adopted (so its record is there to open) but the
        // window opens on the home screen -- what is connected, which folder,
        // the teammates to pick from -- which is what Colin asked launch to be
        // (2026-09-05, twice: "it still opens to the teammates screen with all
        // the chats").
        // The record's phase is always a finished one; what says a run is
        // still under way is the host addressing updates to it.
        const stillLive = response.data.liveMissionIds?.includes(latest.missionId) === true || (pendingUpdatesRef.current.get(latest.runId) ?? []).length > 0
        // Any update addressed to it (a run the host still owns) makes it live.
        setRuns((current) => {
          if (current.size > 0) return current
          const queued = pendingUpdatesRef.current.get(latest.runId) ?? []
          const byId = new Map(response.data.missions.map((mission) => [mission.missionId, mission]))
          return withNewRun(
            current,
            latest.runId,
            queued.reduce(applyMissionUpdate, reopenedRun(latest, byId))
          )
        })
        if (stillLive) setShownKey((current) => current ?? latest.runId)
      })
      .catch(() => {
        // History recovery is optional at startup; discovery remains usable.
      })

    return () => {
      active = false
      clearTimeout(firstRecheck)
      if (rereadSoon !== undefined) clearTimeout(rereadSoon)
      window.removeEventListener('focus', onFocus)
      stopLateWatch()
      window.removeEventListener(SIGN_IN_OPENED_EVENT, onSignInOpened)
      stopSignInClosed()
      removeMissionListener()
      frameBatcher.current.dispose()
      removeApprovalListener()
      removeWithdrawnListener()
      stopUpdates()
    }
  }, [])

  /**
   * Send one reply to a pending request, whichever kind it is.
   *
   * An authorization and an answer travel the same channel and need the same
   * care -- the card only goes when the reply was DELIVERED -- so they share
   * this and differ only in what they carry.
   */
  const replyToApproval = (approvalId: string, reply: Omit<MissionApprovalAnswer, 'approvalId'>): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setDecidingIds((current) => [...current, approvalId])
    void bridge
      .decideMissionApproval({ approvalId, ...reply } as MissionApprovalAnswer)
      .then((response) => {
        // The card goes when the answer was DELIVERED. It used to go whatever
        // happened, so a rejected call left the person believing they had
        // denied a command while the runtime was still waiting to be told.
        // This is the one control here where a dropped answer has a real
        // consequence, so a failure keeps the card and stays clickable.
        if (response.ok) {
          setApprovals((current) => current.filter((entry) => entry.approvalId !== approvalId))
        }
        setDecidingIds((current) => current.filter((entry) => entry !== approvalId))
      })
      .catch(() => {
        setDecidingIds((current) => current.filter((entry) => entry !== approvalId))
      })
  }

  const decideApproval = (approvalId: string, decision: MissionApprovalDecision, reason?: string): void =>
    replyToApproval(approvalId, { decision, ...(reason === undefined ? {} : { reason }) })

  /*
   * WHAT A CARD CAN SAVE AS A RULE (0.521, shared/approval-rules.ts): the
   * exact command, the one file, the one connector tool, for this teammate
   * in this folder. Shown as the sentence it would save; saved by the HOST
   * from the request it holds, which also answers the card.
   */
  const ruleOfCard = (request: MissionApprovalRequest): CardRule | undefined => {
    const teammateId = missionOwners[request.missionId]
    const candidate = ruleCandidateOf(ruledActionOf(request), { ...(teammateId === undefined ? {} : { teammateId }), ...(request.cwd === null ? {} : { folder: request.cwd }) })
    if (candidate === undefined) return undefined
    const name = teammates.find((entry) => entry.teammateId === teammateId)?.name
    const sentence = (effect: 'allow' | 'deny'): string => ruleSentence({ ...candidate, effect, ruleId: '', createdAt: '' }, name)
    return {
      allowSentence: sentence('allow'),
      denySentence: sentence('deny'),
      save: async (effect, reason) => {
        const bridge = window.desktop
        if (bridge === undefined) return 'Locust is not ready yet.'
        setDecidingIds((current) => [...current, request.approvalId])
        try {
          const answer = await bridge.ruleFromApprovalCard({ approvalId: request.approvalId, effect, ...(reason === undefined ? {} : { reason }) }).catch(() => undefined)
          if (answer === undefined) return 'The rule could not be saved. Answer the card as usual.'
          if (!answer.ok) return answer.message
          setApprovals((current) => current.filter((entry) => entry.approvalId !== request.approvalId))
          return undefined
        } finally {
          setDecidingIds((current) => current.filter((entry) => entry !== request.approvalId))
        }
      }
    }
  }

  /** A question's answers, keyed by question id. Never a decision -- see the card. */
  const answerQuestion = (approvalId: string, answers: Readonly<Record<string, readonly string[]>>): void =>
    replyToApproval(approvalId, { answers })

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent): void => {
      const accel = event.ctrlKey || event.metaKey
      // Find in the conversation on screen (0.769): Electron gives a window no find of its own.
      if (accel && !event.shiftKey && !event.altKey && event.key.toLowerCase() === 'f') {
        event.preventDefault()
        setFindRequest((count) => count + 1)
        return
      }
      if (accel && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        setPaletteOpen((open) => !open)
        return
      }
      if (accel && event.key.toLowerCase() === 'i') {
        event.preventDefault()
        setInspectorOpen((open) => !open)
        return
      }
      // The next or previous conversation (2026-10-19, conversationSteps.ts); with Shift, one waiting for you.
      // An unsent message stays with its own conversation: the composer keeps one draft per conversation.
      if (event.altKey && !accel && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
        const rows = [...document.querySelectorAll<HTMLElement>('.lc-conv[data-mission-id]')]
        const drawn = rows.map((row) => row.dataset.missionId ?? '').filter((id) => id.length > 0)
        const shown = rows.find((row) => row.getAttribute('aria-current') === 'true')?.dataset.missionId
        const asking = new Set(approvalsRef.current.map((request) => request.missionId))
        const keysOf = (id: string): readonly string[] => {
          const row = sidebarMissionsRef.current.find((entry) => entry.missionId === id)
          return row === undefined ? [id] : conversationKeys(row)
        }
        const next = stepConversation(drawn, shown, event.key === 'ArrowDown' ? 1 : -1, event.shiftKey ? (id) => keysOf(id).some((key) => asking.has(key)) : undefined)
        if (next !== undefined) {
          event.preventDefault()
          openMissionRef.current(next)
        }
        return
      }
      if (!accel) return
      if (event.key === '1') { event.preventDefault(); setScreen('missions') }
      if (event.key === '2') { event.preventDefault(); setScreen('teammates') }
      // Ctrl 3 reads the storage number too, or the shortcut would be the
      // one way into Settings that still showed the launch reading.
      if (event.key === '3') { event.preventDefault(); refreshStorage(); refreshRuntimeSetup(); refreshWorktrees(); setScreen('settings') }
      if (event.key === '4') { event.preventDefault(); setScreen('rooms') }
      if (event.key === '5') { event.preventDefault(); setScreen('memory') }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  // The first teammate is addressed by default, so a roster of one never
  // needs a click before the first mission; removal falls back the same way.
  const selectedTeammate =
    teammates.find((teammate) => teammate.teammateId === selectedTeammateId) ?? teammates[0]
  /**
   * The teammate the person actually picked, or nobody. `selectedTeammate`
   * falls back to the first teammate so surfaces that need a name have one;
   * a MESSAGE must not inherit that fallback. From the home screen, with no
   * one picked, a message starts a conversation of nobody's -- a plain chat
   * on the route the bar shows -- and "Assign to ..." on its row hands it
   * to a teammate afterwards (Colin, 2026-09-05: no Conversation tab; a
   * mission already is one, a teammate is a saved route with a face).
   */
  const pickedTeammate = addressedTeammate(teammates, selectedTeammateId)
  /*
   * CLOUD IS ONE MESSAGE'S CHOICE, NOT A STATE THE BOX STAYS IN (0.505).
   * Colin, 2026-09-30: picked Cloud to look at it, moved to another
   * conversation, and his next message -- a brief for Sol -- went to Codex
   * Cloud instead of the chat. Another conversation or teammate sets the box
   * back to Direct; so does a cloud task once it has gone.
   */
  useEffect(() => {
    setCloudOn(false)
  }, [shownKey, pickedTeammate?.teammateId])
  // A comparison follows the teammate it was started with (0.441).
  const comparing = compares.find((compare) => compare.compareId === comparingId && compare.teammateId === pickedTeammate?.teammateId)
  /*
   * EVERY COLUMN OF A COMPARISON IS READ WHEN IT IS SHOWN (0.569).
   *
   * History sends the newest twenty turns whole and the rest as rows (H3),
   * and opening a comparison read only the conversation it opened through --
   * the first column. Colin, 2026-10-03, on the day-old arcade-game compare:
   * "disappeared from all models except claude". Cursor's column (the one
   * opened) answered; Codex's and Claude Code's read "No answer was
   * recorded", their ledgers whole on disk. Each column's turns and the
   * judge's are read here, once each.
   */
  const comparedReads = useRef(new Set<string>())
  useEffect(() => {
    const bridge = window.desktop
    if (comparing === undefined || bridge?.readMission === undefined) return
    // A benchmark's runs again too (0.763): their answers and their judges' are read for its table.
    const runsOfIt = (comparing.reruns ?? []).flatMap((id) => compares.filter((one) => one.compareId === id))
    const ids = [comparing, ...runsOfIt].flatMap((one) => [...one.slots.flatMap((slot) => slot.missionIds), ...(one.judge?.missionIds ?? [])])
    const missing = missingTranscripts(ids, historyByIdRef.current).filter((id) => !comparedReads.current.has(id))
    if (missing.length === 0) return
    for (const id of missing) comparedReads.current.add(id)
    void Promise.all(missing.map((id) => bridge.readMission(id).catch(() => undefined))).then((answers) => {
      const read = new Map(
        answers.flatMap((answer) => (answer !== undefined && answer.ok ? [[answer.data.mission.missionId, answer.data.mission] as const] : []))
      )
      // A read that did not answer may be tried again the next time the comparison is shown.
      for (const id of missing) if (!read.has(id)) comparedReads.current.delete(id)
      if (read.size > 0) setHistory((current) => current.map((entry) => read.get(entry.missionId) ?? entry))
    })
  }, [comparing, history, compares])
  const compareMembers = useMemo(() => compareMembership(compares).byMission, [compares])
  /*
   * What each column of a comparison that edits has changed (0.445), read
   * again whenever a column settles: counted in its copy, never while a
   * model is still writing there.
   */
  const settledColumns = comparing?.changes === true && comparing.kept === undefined
    ? comparing.slots
        .map((column) => `${column.slot}:${column.missionIds.join(',')}:${column.missionIds.some((id) => [...runs.values()].some((run) => run.data?.missionId === id && liveRunIsActive(run))) ? 'live' : 'still'}`)
        .join('|')
    : undefined
  useEffect(() => {
    if (comparing === undefined || settledColumns === undefined) return
    const compareId = comparing.compareId
    let active = true
    void window.desktop
      ?.compareChanges(compareId)
      .then((answer) => {
        if (!active || !answer.ok) return
        const columns: Partial<Record<CompareSlotId, string>> = {}
        // A column still writing gets no number yet: it would be out of date as it was drawn.
        const live = new Set(settledColumns.split('|').filter((entry) => entry.endsWith(':live')).map((entry) => entry.split(':')[0]))
        for (const [slot, changes] of Object.entries(answer.data.columns)) {
          if (changes !== undefined && !live.has(slot)) columns[slot as CompareSlotId] = changesLine(changes)
        }
        setCompareChangeLines({ compareId, columns })
      })
      .catch(() => undefined)
    return () => {
      active = false
    }
  }, [comparing?.compareId, settledColumns])
  // Review changes follows the teammate on screen: another conversation never shows this one's branch.
  const reviewing = pickedTeammate !== undefined && pickedTeammate.teammateId === reviewingId && pickedTeammate.worktree === true ? pickedTeammate : undefined
  /*
   * Where a pick is held, and what the conversation on screen ran on (0.552).
   * A run still starting has no record yet, so its new chat's pick holds.
   */
  const pickKey = pickKeyFor(liveRun === undefined ? shownKey : liveRun.data?.missionId, pickedTeammate?.teammateId)
  const conversationRoute = liveRun?.data === undefined ? undefined : { runtime: liveRun.data.runtime, model: liveRun.data.model ?? 'account-default', ...(routeAccount(liveRun.data) === undefined ? {} : { account: routeAccount(liveRun.data) }) }
  const composerRoute = composerRouteFor(route, pickedTeammate, pickerRoutes, pickKey, conversationRoute)
  /*
   * EACH CHAT KEEPS ITS OWN SETUP (0.552). Colin, 10/02: "each time i switch
   * to a new chat it just keeps the mode from the previous chat, there should
   * be persistence in each chat, for permissions, model, chat type, and
   * effort". The model is held above; the mode (Ask, Plan, Edit, Auto...) and
   * the effort are held here, per conversation and per new chat, and put
   * back when it is shown again. A chat never seen before starts on its own
   * defaults: a conversation on what its record says (followRouteOf), a new
   * chat with a teammate on their profile, a new chat with nobody on the
   * last setup used there.
   */
  const setupKey = useRef(pickKey)
  /*
   * A CONVERSATION THAT WENT TO THE CLOUD STAYS THERE (0.761): reopened, its composer has Cloud on when its latest
   * turn ran on the cloud computer -- once per conversation, so Cloud turned off by the person stays off.
   */
  const cloudRestored = useRef(new Set<string>())
  const latestRanInCloud = liveRun?.data?.missionId !== undefined && cloudTurns.has(liveRun.data.missionId)
  useEffect(() => {
    if (!latestRanInCloud || cloudRestored.current.has(pickKey)) return
    cloudRestored.current.add(pickKey)
    if (composerRoute.runtime === 'opencode') setCloudOn(true)
    else if (composerRoute.runtime === 'claude' || composerRoute.runtime === 'codex') setComputerOn(true)
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once per conversation, as it is opened
  }, [latestRanInCloud, pickKey])
  useEffect(() => {
    if (setupKey.current !== pickKey) {
      setupKey.current = pickKey
      const held = chatSetups.current.get(pickKey)
      // Never seen this session: a new chat's mode and effort from its
      // defaults; a conversation's from its record (already set on opening).
      const fresh = pickKey === 'new' ? newChatSetup : pickKey.startsWith('new:') ? (pickedTeammate?.route === undefined ? undefined : { mode: pickedTeammate.route.mode, effort: pickedTeammate.route.effort }) : undefined
      // The chat mode too: a chat never seen is Direct, whatever the last one was.
      const kind = held?.kind ?? { compare: false, blind: false, cloud: false }
      const setup = held ?? (fresh === undefined ? undefined : { ...fresh, kind })
      let moved = false
      if (kind.compare !== compareOn || kind.blind !== compareBlind || kind.cloud !== cloudOn || (kind.computer === true) !== computerOn) {
        setCompareOn(kind.compare)
        setCompareBlind(kind.blind)
        setCloudOn(kind.cloud)
        setComputerOn(kind.computer === true)
        moved = true
      }
      if (setup !== undefined && (setup.mode !== mode || setup.effort !== effort)) {
        if (modeRunsOn(setup.mode, composerRoute.runtime, build?.platform)) setMode(setup.mode)
        setEffort(setup.effort)
        moved = true
      }
      if (moved) return
    }
    chatSetups.current.set(pickKey, { mode, effort, kind: { compare: compareOn, blind: compareBlind, cloud: cloudOn, computer: computerOn } })
    if (pickKey === 'new' && newChatSetup !== undefined && (newChatSetup.mode !== mode || newChatSetup.effort !== effort)) {
      rememberNewChatRoute(composerRoute, mode, effort)
    }
  }, [pickKey, mode, effort, compareOn, compareBlind, cloudOn, computerOn])

  /** Who a run belongs to: what it was started with, or what the host recorded. */
  const ownerOf = (run: LiveRunState): string | undefined =>
    run.teammateId ?? (run.data === undefined ? undefined : missionOwners[run.data.missionId])
  /*
   * WHO TALK CALLS (0.746): the teammate picked in this window, and the conversation on screen when it is theirs.
   * Nobody picked -- Home, a chat of nobody's -- and Talk is ChatGPT's own voice, as before.
   */
  const talkTo = pickedTeammate === undefined
    ? undefined
    : {
        teammateId: pickedTeammate.teammateId,
        name: pickedTeammate.name,
        hue: pickedTeammate.hue,
        avatar: pickedTeammate.avatar,
        ...(liveRun?.data !== undefined && ownerOf(liveRun) === pickedTeammate.teammateId ? { conversationId: liveRun.data.missionId } : {})
      }
  const talkToRef = useRef(talkTo)
  talkToRef.current = talkTo

  // Said before the send: a reply on another runtime continues from the
  // stopped run's checkpoint, not from its memory (0.21.2 QA, P2).
  const shownData = liveRun?.data
  const runtimeNameOf = (id: string): string => runtimes.find((entry) => entry.id === id)?.displayName ?? id
  const continuationNote =
    liveRun !== undefined
    && shownData !== undefined
    && !liveRunIsActive(liveRun)
    && shownData.runtime !== composerRoute.runtime
    && ownerOf(liveRun) === pickedTeammate?.teammateId
    // A cloud task does not continue this conversation (Sol's 0.504 pass): no hint about its checkpoint.
    && !cloudOn
    // Nor does an ask in an undecided comparison: it goes to every column, on its own model (0.520).
    && !(comparing !== undefined && comparing.kept === undefined)
      // Plain words (0.517): "checkpoint" and "briefed" were the app's, not the person's.
      ? `Your next message goes to ${runtimeNameOf(composerRoute.runtime)} with a summary of this conversation, not ${runtimeNameOf(shownData.runtime)}'s memory of it.`
      : undefined
  /*
   * A reply addressed to someone who does not own the conversation on screen
   * starts them a new one -- on purpose, but it was said nowhere until the
   * new row appeared (0.567, Colin: "it started a whole new chat").
   */
  const shownOwner = liveRun === undefined ? undefined : ownerOf(liveRun)
  const newConversationNote =
    liveRun !== undefined
    && shownData !== undefined
    && !liveRunIsActive(liveRun)
    && pickedTeammate !== undefined
    && shownOwner !== undefined
    && shownOwner !== pickedTeammate.teammateId
    && !cloudOn
    && comparing === undefined
      ? `Your next message starts a new conversation with ${pickedTeammate.name}. This one stays with ${teammates.find((mate) => mate.teammateId === shownOwner)?.name ?? 'its teammate'}.`
      : undefined
  // What that summary carries, asked of the host as the reply is typed (0.517, Composer).
  const continuation = continuationNote === undefined || shownData === undefined
    ? undefined
    : { followUpOf: shownData.missionId, runtime: composerRoute.runtime }

  /**
   * The exchange the shown conversation is part of, read from what the
   * window already holds: every recovered mission plus every live run,
   * linked by the workroom messages they posted and received. Nothing is
   * bookkept twice -- the strip says what the records say.
   */
  /**
   * The turns of this conversation that changed the file now open, oldest
   * first, for the viewer's history strip.
   *
   * Memoised because it walks every turn's events through `buildThread` --
   * cheap for a conversation, wasteful on every frame of a streaming reply,
   * and the viewer is open across exactly those frames.
   */
  const viewingFileTurns = useMemo(() => {
    if (viewingFile === undefined || liveRun === undefined) return []
    const turns = [
      ...(liveRun.earlierTurns ?? []).map((turn) => ({
        missionId: turn.missionId,
        prompt: turn.prompt,
        events: turn.events
      })),
      {
        missionId: liveRun.data?.missionId ?? liveRun.restoredMission?.missionId ?? 'live',
        prompt: liveRun.prompt,
        events: liveRun.events
      }
    ]
    return fileTurns(turns, viewingFile.path, workspacePath)
  }, [viewingFile, liveRun, workspacePath])
  /**
   * Every mission the window knows, live or recorded, in one shape.
   *
   * Lifted out of the exchange memo because a ROOM needs the same thing: a
   * post's conversation is an exchange rooted at the missions that answered
   * it, and walking it needs every mission's peer messages.
   */
  const missionsForExchange = useMemo(() => {
    const byMission = new Map<string, ExchangeMission>()
    for (const mission of history) {
      byMission.set(mission.missionId, {
        missionId: mission.missionId,
        runtime: mission.runtime,
        model: mission.model,
        events: mission.events,
        peerMessages: mission.peerMessages,
        ...(mission.startedBy === undefined ? {} : { startedBy: mission.startedBy }),
        ...(missionOwners[mission.missionId] === undefined ? {} : { teammateId: missionOwners[mission.missionId] }),
        ...(teammates.find((entry) => entry.teammateId === missionOwners[mission.missionId])?.name === undefined
          ? {}
          : { teammateName: teammates.find((entry) => entry.teammateId === missionOwners[mission.missionId])!.name }),
        live: false
      })
    }
    // Live runs overlay the record: fresher events, and which are still going.
    for (const run of runs.values()) {
      if (run.data === undefined) continue
      const owner = ownerOf(run)
      const name = teammates.find((entry) => entry.teammateId === owner)?.name
      // Merge over the record rather than replace it: a run reopened from the
      // ledger carries its events but not always who started it, and the
      // strip's hop count read 0 for a relayed reply until this kept the
      // record's answer (exchange smoke, first run).
      const recorded = byMission.get(run.data.missionId)
      const startedBy = run.startedBy ?? recorded?.startedBy
      byMission.set(run.data.missionId, {
        missionId: run.data.missionId,
        runtime: run.data.runtime,
        model: run.data.model,
        events: run.events.length > 0 ? run.events : recorded?.events ?? [],
        peerMessages: run.peerMessages ?? recorded?.peerMessages ?? [],
        ...(startedBy === undefined ? {} : { startedBy }),
        ...(owner === undefined ? {} : { teammateId: owner }),
        ...(name === undefined ? {} : { teammateName: name }),
        live: liveRunIsActive(run),
        runId: run.data.runId
      })
    }
    return byMission
  }, [history, runs, missionOwners, teammates])

  const exchange = useMemo(
    () => (shownData === undefined ? undefined : exchangeOf(shownData.missionId, [...missionsForExchange.values()])),
    [shownData, missionsForExchange]
  )

  /**
   * A post, as the conversation it became -- or nothing, when it stayed a set
   * of independent answers and the grid of cards is the truth.
   *
   * Rooted at EVERY mission the post started, not the one that reaches
   * furthest: a reply is a new mission linked only to the turn it answers, so
   * two members who answer at once and then write to each other leave two
   * halves of one argument. Walking from one of them drew the argument with
   * its opening turn missing (MEASURED 2026-09-11).
   */
  const roomExchangeFor = (room: PublicRoom, postId: string): RoomExchange | undefined => {
    const post = room.posts.find((entry) => entry.postId === postId)
    if (post === undefined) return undefined
    const whole = exchangeAcross(Object.values(post.missions), [...missionsForExchange.values()])
    if (whole === undefined) return undefined
    // The newest post that is still older than nothing: anything said after
    // it, under THIS post, is the case that needs a time.
    const laterPostAt = room.posts
      .filter((entry) => entry.at > post.at)
      .map((entry) => entry.at)
      .sort()[0]
    const textOf = (missionId: string): string | undefined => {
      const live = [...runs.values()].find((run) => run.data?.missionId === missionId)
      const recorded = historyByIdRef.current.get(missionId)
      const events = live !== undefined && live.events.length > 0 ? live.events : recorded?.events ?? []
      /*
       * EVERY message of the turn, not the last one.
       *
       * This took `finals.at(-1)` -- the last message marked final -- and fell
       * back to the latest message while none was. So a teammate's progress
       * appeared as it was written and then VANISHED the moment the turn
       * finished: the fallback had been showing message three, and the final
       * replaced the lot with message four. Colin, 2026-09-13: "the agents
       * initial messages are properly appearing in rooms chat but then when
       * final message is sent out it disappears."
       *
       * The same defect the answer cards had before 0.89.0, surviving in the
       * path that kept the old shape: one string standing in for a whole
       * turn. The thread has always drawn all of them, which is why the
       * mission opened beside the room showed four messages where the room
       * showed one.
       */
      const raw = turnText(events)
      // Trimmed: a stripped share block leaves the blank lines that held it,
      // and the room drew a turn whose name and first sentence were an inch
      // apart for no reason a reader could see (MEASURED 2026-09-11).
      const said = stripFileBlocks(stripMemoryBlocks(stripTaskBlocks(stripDecisionBlocks(stripShareBlocks(raw))))).trim()
      return said.length === 0 ? undefined : said
    }
    return sequenceOfPost({
      post: { postId: post.postId, at: post.at, missions: post.missions, ...(post.queued === undefined ? {} : { waiting: post.queued }) },
      missions: missionsForExchange,
      reached: whole.missionIds,
      textOf,
      startedAtOf: (missionId) =>
        [...runs.values()].find((run) => run.data?.missionId === missionId)?.startedAtIso
        ?? historyByIdRef.current.get(missionId)?.createdAt,
      finishedOf: (missionId) => {
        const live = [...runs.values()].find((run) => run.data?.missionId === missionId)
        return live === undefined ? true : !liveRunIsActive(live)
      },
      nameOf: (teammateId) => teammates.find((entry) => entry.teammateId === teammateId)?.name ?? teammateId,
      laterPostAt,
      hops: whole.hops,
      cap: relayHopCap,
      cost: whole.cost,
      starting: Object.values(relayStarting),
      liveOf: liveTurnOf
    })
  }

  const roomExchangeCostText = (room: PublicRoom, postId: string): string | undefined => {
    const found = roomExchangeFor(room, postId)
    return found === undefined ? undefined : costLine(found.foot.cost)
  }

  /**
   * The answers under one room post, read from the missions it started:
   * the live run when the window holds it, else the record. The phase is
   * the record's own word and the text is the teammate's last final
   * message -- nothing summarised, and every card opens its mission.
   */
  const roomAnswersFor = (room: PublicRoom, postId: string): readonly RoomAnswer[] => {
    const post = room.posts.find((entry) => entry.postId === postId)
    if (post === undefined) return []
    const answers: RoomAnswer[] = []
    for (const [teammateId, missionId] of Object.entries(post.missions)) {
      const live = [...runs.values()].find((run) => run.data?.missionId === missionId)
      const recorded = historyByIdRef.current.get(missionId)
      const events = live !== undefined && live.events.length > 0 ? live.events : recorded?.events ?? []
      // Same rule as the exchange above: a turn is everything it said.
      const raw = turnText(events)
      // The words, not the blocks: what a reply shared, asked or moved on the
      // board is shown by those surfaces. The room smoke's first live run
      // drew a raw task block inside the card (2026-09-05).
      const last = raw === undefined ? undefined : stripFileBlocks(stripMemoryBlocks(stripTaskBlocks(stripDecisionBlocks(stripShareBlocks(raw)))))
      const phase = live !== undefined ? live.phase : recorded?.phase ?? 'unknown'
      const runtime = live?.data?.runtime ?? recorded?.runtime ?? 'codex'
      const model = live?.data?.model ?? recorded?.model ?? 'account-default'
      answers.push({
        teammateId,
        missionId,
        /*
         * When the asking started, so the card can say how long it has been
         * quiet. The first event is the honest mark for a run that has
         * spoken; for one that has not, the post is -- which is exactly the
         * window this is for. A queued member's clock starts when they were
         * asked, not when the post was made, and their first event is the
         * only record of that.
         */
        startedAt: events[0]?.occurredAt ?? post.at,
        phase,
        text: last === undefined || last.trim().length === 0 ? undefined : last,
        /*
         * The whole turn, drawn by the thread's own renderer.
         *
         * Built here rather than in the room because this is where the events
         * are -- live if the run is still going, from the record otherwise --
         * and because `buildThread` is the one place that decides what a turn
         * looks like. A second opinion about that in the room is how the two
         * surfaces came to disagree in the first place.
         */
        items: buildThread(events, {
          running: phase === 'running' || phase === 'starting',
          trimmed: live !== undefined && live.events.length > 0 ? live.eventsTruncated === true : recorded?.eventsTruncated === true,
          mayEdit: (live?.data?.sandbox ?? recorded?.sandbox) !== 'read-only',
          ...(workspacePath === undefined ? {} : { workspacePath })
        }),
        runtime,
        model
      })
    }
    return answers
  }

  const refreshRooms = (): void => {
    void window.desktop
      ?.listRooms()
      .then((response) => {
        noteStore('rooms', response.ok)
        if (response.ok) setRooms(response.data.rooms)
      })
      .catch(() => undefined)
  }

  /** Every memory answer carries the whole list; adopt it, or hand back the refusal. */
  const adoptMemories = (response: MemoryListResponse): string | undefined => {
    if (!response.ok) return response.error.message
    setMemories(response.data.memories)
    setForgottenMemories(response.data.forgotten ?? [])
    setMemoriesOutOfDate(response.data.changedSince ?? {})
    setBriefTrackingSince(response.data.briefTrackingSince)
    setMemoryWorkspace({ id: response.data.workspaceId, name: response.data.workspaceName })
    return undefined
  }
  useEffect(() => {
    if (screen === 'memory') refreshMemories()
    /*
     * And every open menu closes with the screen it belongs to.
     *
     * Grok's beta drive, 2026-09-14, finding 7: the room header's menu was
     * opened -- All rooms / Rename / Remove for good? -- and then Ctrl+2 and
     * Ctrl+3 switched screens with the keyboard. The menu floated over Team
     * and over Home, still offering to remove a room the person was no longer
     * looking at.
     *
     * Dismiss-on-click-outside cannot catch this: no click happened. A menu
     * is bound to the thing it was opened from, and that thing is gone.
     */
    setRowMenu(undefined)
    setRowMenuArmed(undefined)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [screen])
  // Leaving a room is the same event as leaving a screen, as far as a menu
  // that belongs to that room is concerned.
  useEffect(() => {
    setRowMenu(undefined)
    setRowMenuArmed(undefined)
  }, [currentRoomId])
  const refreshMemories = (): void => {
    void window.desktop
      ?.listMemories()
      .then(adoptMemories)
      .catch(() => undefined)
  }
  const memoryCall = async (call: Promise<MemoryListResponse> | undefined): Promise<string | undefined> => {
    if (call === undefined) return 'Memory is not available here.'
    try {
      const response = await call
      // A refusal can still have changed the list -- a suggestion refused as
      // out of date is dropped (A1.2) -- so the screen reads it again.
      if (!response.ok) refreshMemories()
      return adoptMemories(response)
    } catch {
      return 'Memory could not be changed. What was saved is still saved.'
    }
  }
  const addMemory = (text: string, scope: MemoryScope): Promise<string | undefined> => memoryCall(window.desktop?.addMemory({ text, scope }))
  const updateMemory = (request: MemoryUpdateRequest): Promise<string | undefined> => memoryCall(window.desktop?.updateMemory(request))
  const removeMemory = (memoryId: string): Promise<string | undefined> => memoryCall(window.desktop?.removeMemory(memoryId))
  const clearMemories = (scope: 'workspace' | 'all'): Promise<string | undefined> => memoryCall(window.desktop?.clearMemories({ scope }))
  const restoreMemory = (memoryId: string): Promise<string | undefined> => memoryCall(window.desktop?.restoreMemory(memoryId))
  const rememberNewChatRoute = (next: RouteChoice, nextMode: MissionMode, nextEffort: string | undefined): void => {
    const newChatRoute = { runtime: next.runtime, model: next.model, ...(next.account === undefined ? {} : { account: next.account }), mode: nextMode, ...(nextEffort === undefined ? {} : { effort: nextEffort }) }
    setNewChatSetup({ mode: nextMode, effort: nextEffort })
    void window.desktop
      ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube, newChatRoute })
      .catch(() => undefined)
  }
  const changeMemoryMode = (next: MemoryMode): void => {
    const before = memoryMode
    setMemoryMode(next)
    void window.desktop
      ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode: next, autoMode, askConnectors, keepATodoList, replySize, layout, tube })
      .then((settings) => setMemoryMode(settings.memoryMode))
      .catch(() => setMemoryMode(before))
  }

  /**
   * Make a room and say which one, because the caller usually wants to post
   * to it next.
   *
   * `createRoom` below keeps the old shape -- an error string or nothing --
   * for the Rooms screen's own form, which has nothing to do afterwards.
   */
  const makeRoom = async (
    name: string,
    teammateIds: readonly string[]
  ): Promise<{ readonly roomId: string } | { readonly message: string }> => {
    const bridge = window.desktop
    if (bridge === undefined) return { message: 'The secure desktop bridge is unavailable.' }
    const response = await bridge.createRoom({ name, teammateIds }).catch(() => undefined)
    if (response === undefined) return { message: 'The room could not be created. No room was added.' }
    if (!response.ok) return { message: response.error.message }
    refreshRooms()
    const made = response.data.room
    if (made === undefined) return { message: 'The room could not be created. No room was added.' }
    setCurrentRoomId(made.roomId)
    return { roomId: made.roomId }
  }

  const createRoom = async (name: string, teammateIds: readonly string[]): Promise<string | undefined> => {
    const made = await makeRoom(name, teammateIds)
    return 'roomId' in made ? undefined : made.message
  }

  const renameRoom = (roomId: string, name: string): void => {
    void window.desktop
      ?.renameRoom(roomId, name)
      .then((response) => {
        if (!response.ok) {
          setRoomNotice(response.error.message)
          return
        }
        setRoomNotice(undefined)
        refreshRooms()
      })
      .catch(() => setRoomNotice('That room could not be renamed. It kept the name it had.'))
  }

  const removeRoom = (roomId: string): void => {
    void window.desktop
      ?.removeRoom(roomId)
      .then((response) => {
        if (!response.ok) {
          setRoomNotice(response.error.message)
          return
        }
        setCurrentRoomId((current) => (current === roomId ? undefined : current))
        refreshRooms()
      })
      .catch(() => setRoomNotice('That room could not be removed. It and its posts are still there.'))
  }

  const postToRoom = async (roomId: string, text: string, to?: readonly string[]): Promise<string | undefined> => {
    const bridge = window.desktop
    if (bridge === undefined) return 'The secure desktop bridge is unavailable.'
    // `to`: the members the person named (0.371); absent asks everyone.
    const response = await bridge.postToRoom({ roomId, text, ...(to === undefined || to.length === 0 ? {} : { to }) }).catch(() => undefined)
    if (response === undefined) return 'The post could not be made. Nothing was added to the room.'
    if (!response.ok) return response.error.message
    refreshRooms()
    // Who could not be started, in the host's words, said once per reason.
    setRoomNotice(refusalNotice(response.data.refused))
    return undefined
  }

  /** A person moving a room's board. The host answers with the room as it now stands. */
  const updateRoomTask = async (request: RoomTaskRequest): Promise<string | undefined> => {
    const bridge = window.desktop
    if (bridge === undefined) return 'The secure desktop bridge is unavailable.'
    const response = await bridge.updateRoomTask(request).catch(() => undefined)
    if (response === undefined) return 'The board could not be changed. It still shows what it showed.'
    if (!response.ok) return response.error.message
    setRooms((current) => current.map((room) => (room.roomId === response.data.room.roomId ? response.data.room : room)))
    return undefined
  }

  /** Stop every run in the exchange. Records stay; only the processes go. */
  const stopExchange = (runIds: readonly string[]): void => {
    const bridge = window.desktop
    if (bridge === undefined || runIds.length === 0) return
    setStoppingExchange(true)
    void Promise.all(
      runIds.map((runId) => {
        setRuns((all) => withRun(all, runId, markedCancelling))
        return bridge.cancelCodexMission({ runId }).catch(() => undefined)
      })
    ).finally(() => setStoppingExchange(false))
  }

  /**
   * M27: why the last start went back to the box, in the host's words, for
   * the composer to say. A send refused as busy with nothing of the
   * teammate's to wait behind -- a conversation of nobody's while another
   * runs, or every slot taken -- came back into the box with no reason.
   */
  const startRefusal = useRef<string | undefined>(undefined)
  /** M28: why the last Resume from checkpoint did not start, for the card that was pressed. */
  const [resumeRefusal, setResumeRefusal] = useState<{ readonly missionId: string; readonly message: string }>()
  /*
   * COMPARE (0.441, shared/compare.ts, docs/PLAN-2026-09-28-COMPARE.md).
   *
   * The picker's One / Compare switch lives in a teammate's conversation;
   * the ask then goes to every ticked model, each its own mission, and the
   * comparison takes the thread's place until one is kept.
   */
  const samePick = (a: RouteChoice, b: RouteChoice): boolean => a.runtime === b.runtime && a.model === b.model && a.account === b.account
  // Anywhere: no teammate is needed to compare (Colin, 2026-09-28), so a new person can try it first thing.
  // Above the comparison's default picks, which read both while rendering (0.460).
  // What each route's model turned out to be, from missions that already ran.
  const resolvedModels = useMemo(() => resolvedModelNames(history), [history])
  const recentRoutes = useMemo(() => recentlyUsedRoutes(history), [history])
  // A comparison answers read-only; a model that cannot be held read-only here answers in a copy (0.443).
  // One that edits runs in Edit, in its own copy (0.445).
  const compareChoiceRefusal = (choice: { readonly runtime: ComparePick['runtime'] }, changes: boolean = compareChanges): string | undefined =>
    compareRefusalOf(choice.runtime, antigravityCli)
    ?? (changes
      ? modeRunsOn('accept-edits', choice.runtime, build?.platform) ? undefined : `${modeUnavailableReason('accept-edits', choice.runtime, build?.platform) ?? 'It cannot edit here.'} So it cannot join a comparison that edits.`
      : undefined)
    ?? (changes || modeRunsOn('ask', choice.runtime, build?.platform) || compareNeedsCopy(choice.runtime, build?.platform)
      ? undefined
      : `${modeUnavailableReason('ask', choice.runtime, build?.platform) ?? 'It cannot answer read-only here.'} A comparison answers read-only, so it cannot join one.`)
  /** The two a new comparison starts on (compareDefaults.ts), as Arena opens Side by Side (0.460). */
  const startingPicks = (changes: boolean = compareChanges): readonly ComparePick[] =>
    defaultComparePicks({
      // The box's effort comes in with its model (0.550, Sol on 0.546: a
      // conversation on Low opened Compare on High). The route carries none;
      // the level is the box's own, when its model has that level.
      current: effort !== undefined && (modelFamily(models, composerRoute.runtime, composerRoute.model)?.supportedEfforts ?? []).includes(effort)
        ? { ...composerRoute, effort }
        : composerRoute,
      recent: recentRoutes,
      answered: freeAnswered,
      down: freeDown,
      models,
      ready: (runtime) => runtimes.some((status) => status.id === runtime && status.ready),
      refusal: (choice) => compareChoiceRefusal(choice, changes),
      label: (choice) => routeModelName(choice.runtime, choice.model, resolvedModels.get(`${choice.runtime}:${choice.model}`))
    })
  const comparePicking: ComparePicking | undefined = runtimes.length === 0
    ? undefined
    : {
        on: compareOn,
        picks: comparePicks,
        onMode: (on) => {
          setCompareOn(on)
          if (!on) {
            setComparePicks([])
            setCompareChanges(false)
          } else if (comparePicks.length < MIN_COMPARE_SLOTS) {
            const starting = startingPicks()
            if (starting.length >= MIN_COMPARE_SLOTS) setComparePicks(starting)
          }
        },
        prefills: comparePicks.length >= MIN_COMPARE_SLOTS || startingPicks().length >= MIN_COMPARE_SLOTS,
        onReplace: (index, pick) =>
          setComparePicks((current) => current.map((one, at) => (at === index ? pick : one))),
        onEffort: (index, effort) =>
          setComparePicks((current) => current.map((one, at) => (at === index ? { ...one, effort } : one))),
        changes: compareChanges,
        onChanges: setCompareChanges,
        ...(compareChangesRefusal === undefined ? {} : { changesRefusal: compareChangesRefusal }),
        ...(compareInPlace ? { inPlace: true } : {}),
        blind: compareBlind,
        onBlind: setCompareBlind,
        record: compareRecord(compares),
        // Any folder: a git project gives each model a worktree, any other a plain copy (0.448).
        onToggle: (pick) =>
          setComparePicks((current) =>
            current.some((one) => samePick(one, pick))
              ? current.filter((one) => !samePick(one, pick))
              : current.length >= MAX_COMPARE_SLOTS ? current : [...current, pick]
          ),
        // A comparison answers read-only; a model that cannot be held read-only here answers in a copy (0.443).
        // One that edits runs in Edit, in its own copy (0.445).
        refusal: (choice) => compareChoiceRefusal(choice)
      }
  const replaceCompare = (next: PublicCompare): void =>
    setCompares((current) => [...current.filter((one) => one.compareId !== next.compareId), next])

  /** The send, when a comparison is ticked or on screen; otherwise the ordinary one. */
  const openTune = (): string | undefined => {
    if (window.desktop === undefined) return 'Locust is not ready. Nothing was started.'
    if (tuning !== undefined) return 'The tune card is already open. Nothing else was started.'
    const teammate = liveRun === undefined ? undefined : teammates.find((mate) => mate.teammateId === ownerOf(liveRun))
    const of = liveRun?.data?.missionId
    const why = tuneRefusal(teammate, of, liveRun !== undefined && isTerminal(liveRun.phase) && comparing === undefined)
    if (why !== undefined || teammate?.route === undefined || of === undefined) return why ?? 'Open a finished teammate conversation first. Nothing was started.'
    const id = ++tuneGeneration.current
    const current = teammate.instructions ?? ''
    const route = teammate.route
    setTuning({ id, teammateId: teammate.teammateId, route, name: teammate.name, model: routeModelName(route.runtime, route.model), free: route.runtime === 'opencode' && /(^|[-/])free$/i.test(route.model), prompt: liveRun!.prompt, current, drafts: currentDrafts(current, 'Drafting an alternative on this teammate’s model…'), drafting: true })
    void window.desktop?.draftTune(of).then((answer) => {
      if (tuneGeneration.current !== id) {
        if (answer.ok) void window.desktop?.cancelCodexMission({ runId: answer.data.runId })
        return
      }
      setTuning((card) => card?.id !== id ? card : answer.ok
        ? { ...card, draftRunId: answer.data.runId }
        : { ...card, drafting: false, drafts: currentDrafts(card.current), problem: answer.message })
    }).catch(() => setTuning((card) => card?.id !== id ? card : { ...card, drafting: false, drafts: currentDrafts(card.current) }))
    return undefined
  }
  const closeTune = (): void => {
    if (tuning?.starting) return
    tuneGeneration.current += 1
    if (tuning?.draftRunId !== undefined && tuning.drafting) void window.desktop?.cancelCodexMission({ runId: tuning.draftRunId })
    setTuning(undefined)
  }
  const runTune = async (): Promise<void> => {
    const card = tuning
    const bridge = window.desktop
    if (card === undefined || card.drafting || card.starting || bridge === undefined) return
    const teammate = teammates.find((mate) => mate.teammateId === card.teammateId)
    if (teammate?.route === undefined || JSON.stringify(teammate.route) !== JSON.stringify(card.route)) { setTuning({ ...card, problem: 'That teammate’s model changed. Cancel and open /tune again to review it. Nothing was started.' }); return }
    setTuning({ ...card, starting: true })
    const route = teammate.route
    const answer = await bridge.startCompare({ teammateId: teammate.teammateId, prompt: card.prompt,
      routes: tuneRoutes({ runtime: route.runtime, model: route.model, ...(route.account === undefined ? {} : { account: route.account }), ...(route.effort === undefined ? {} : { effort: route.effort }), label: card.model }, card.current, card.drafts), changes: true, blind: true }).catch(() => undefined)
    if (answer?.ok !== true) { setTuning({ ...card, starting: false, problem: answer?.ok === false ? answer.error.message : 'The trials could not be started. Nothing was changed.' }); return }
    replaceCompare(answer.data.compare)
    setComparingId(answer.data.compare.compareId)
    setCompareOn(false)
    setTuning(undefined)
  }
  const useTunedInstructions = async (teammate: PublicTeammate, instructions: string): Promise<string | undefined> => {
    const bridge = window.desktop
    if (bridge === undefined) return 'Locust is not ready. The teammate is unchanged.'
    // Read identity fresh: saving only standing words must not undo a concurrent edit to the teammate.
    const listed = await bridge.listTeammates()
    const latest = listed.ok ? listed.data.teammates.find((mate) => mate.teammateId === teammate.teammateId) : undefined
    if (latest === undefined) return 'That teammate is no longer available. Nothing was changed.'
    const answer = await bridge.updateTeammate({ teammateId: latest.teammateId, name: latest.name, hue: latest.hue, role: latest.role,
      ...(latest.roleTitle === undefined ? {} : { roleTitle: latest.roleTitle }), ...(latest.worktree === undefined ? {} : { worktree: latest.worktree }), avatar: latest.avatar, instructions: instructions.length === 0 ? null : instructions })
    if (!answer.ok) return answer.error.message
    const refreshed = await bridge.listTeammates()
    if (refreshed.ok) { setTeammates(refreshed.data.teammates); setMissionOwners(refreshed.data.missionOwners); rememberRosterNames(refreshed.data) }
    return undefined
  }

  const sendOrCompare = async (prompt: string, sendOptions?: { readonly leaveOut?: readonly string[]; readonly referenceIds?: readonly string[] }): Promise<boolean | string> => {
    if (prompt.trim().toLowerCase() === '/tune') return openTune() ?? true
    const bridge = window.desktop
    // Background (W10): Claude Code's own session takes the turn; the panel follows it.
    if (backgroundOn && comparing === undefined && composerRoute.runtime === 'claude') {
      if (!bridge) return 'Locust is not ready yet. Nothing was sent.'
      const backgroundEffort = effort !== undefined && (modelFamily(models, 'claude', composerRoute.model)?.supportedEfforts ?? []).includes(effort) ? effort : undefined
      const continuing = liveRun?.data?.runtime === 'claude' ? liveRun.data.missionId : undefined
      const sent = await bridge.startBackground({
        prompt: claimCommands(prompt, draftKeyRef.current),
        ...(composerRoute.account === undefined ? {} : { account: composerRoute.account }),
        mode,
        model: composerRoute.model,
        ...(backgroundEffort === undefined ? {} : { effort: backgroundEffort }),
        ...(pickedTeammate === undefined ? {} : { teammateId: pickedTeammate.teammateId }),
        ...(continuing === undefined ? {} : { conversation: continuing })
      }).catch(() => undefined)
      setBackgroundPanel(true)
      if (sent === undefined) {
        setBackgroundNote({ text: 'Claude Code could not be reached. Nothing was sent.' })
        return 'Claude Code could not be reached. Nothing was sent.'
      }
      if (!sent.ok) {
        setBackgroundNote({ text: sent.message, ...(sent.needsTrust === true ? { setUp: true as const } : {}) })
        return sent.message
      }
      setBackgroundNote(undefined)
      setBackgroundRuns((current) => [sent.run, ...current.filter((run) => run.id !== sent.run.id)])
      setBackgroundOn(false)
      return true
    }
    // YOUR CLOUD COMPUTER (0.761): an OpenCode teammate's turn runs on the repository's codespace, as a turn of this
    // conversation. Cloud stays on, so the conversation's next turn goes there too.
    if (cloudOn && comparing === undefined && composerRoute.runtime === 'opencode') {
      return startFromComposer(prompt, sendOptions?.leaveOut, true, sendOptions?.referenceIds)
    }
    // Claude Code or Codex on your own cloud computer (0.768), not its maker's cloud: a turn of this conversation there.
    if (computerOn && comparing === undefined && (composerRoute.runtime === 'claude' || composerRoute.runtime === 'codex')) {
      return startFromComposer(prompt, sendOptions?.leaveOut, true, sendOptions?.referenceIds)
    }
    // Cloud (0.503): the task goes to Codex Cloud, and the panel follows it.
    if (cloudOn && comparing === undefined && composerRoute.runtime === 'claude') {
      if (!bridge) return 'Locust is not ready yet. Nothing was sent.'
      // On the model and effort in the box (0.557): without them, every session ran on the account's default.
      const cloudEffort = effort !== undefined && (modelFamily(models, 'claude', composerRoute.model)?.supportedEfforts ?? []).includes(effort) ? effort : undefined
      const environment = cloudEnvironment.folder === workspacePath ? cloudEnvironment.value : ''
      const sent = await bridge.startClaudeCloud(claimCommands(prompt, draftKeyRef.current), pickedTeammate?.teammateId, { model: composerRoute.model, ...(cloudEffort === undefined ? {} : { effort: cloudEffort }), ...(environment.length === 0 ? {} : { environment }) }).catch(() => undefined)
      if (sent === undefined) return 'Claude Code could not be opened. Nothing was sent.'
      if (!sent.ok) return sent.message
      setClaudeCloud((current) => [sent.session, ...current.filter((one) => one.id !== sent.session.id)])
      setClaudeCloudNote(undefined)
      setCloudPanel(true)
      setCloudOn(false)
      return true
    }
    if (cloudOn && comparing === undefined) {
      if (!bridge) return 'Locust is not ready yet. Nothing was sent.'
      const started = await bridge.startCloudTask(claimCommands(prompt, draftKeyRef.current), pickedTeammate?.teammateId).catch(() => undefined)
      if (started === undefined) return 'The cloud task could not be started. Nothing was sent.'
      if (!started.ok) return started.message
      setCloudTasks((current) => [...current.filter((task) => task.taskId !== started.task.taskId), started.task])
      setCloudNotes(started.notes)
      setCloudProblem(undefined)
      setCloudPanel(true)
      setCloudOn(false)
      return true
    }
    if (comparing !== undefined && comparing.kept === undefined) {
      if (!bridge) return 'Locust is not ready yet. Nothing was sent.'
      const answer = await bridge.askCompare(comparing.compareId, claimCommands(prompt, draftKeyRef.current)).catch(() => undefined)
      if (answer === undefined) return 'The comparison could not be asked. Nothing was started.'
      if (!answer.ok) return answer.error.message
      replaceCompare(answer.data.compare)
      const asked = answer.data.compare.slots.filter((column) => column.refused === undefined && column.missionIds.length > 0).length
      // Every column refused: the words come back with the reason.
      return answer.data.refused.length >= asked && answer.data.refused[0] !== undefined ? answer.data.refused[0].message : true
    }
    // A kept comparison on screen: the conversation it became is where this goes.
    if (comparing !== undefined) setComparingId(undefined)
    if (compareOn && comparePicks.length > 0) {
      if (comparePicks.length < MIN_COMPARE_SLOTS) return 'Pick one more model to compare with, or switch the picker back to One.'
      if (!bridge) return 'Locust is not ready yet. Nothing was sent.'
      const answer = await bridge
        .startCompare({
          ...(pickedTeammate === undefined ? {} : { teammateId: pickedTeammate.teammateId }),
          prompt: claimCommands(prompt, draftKeyRef.current),
          // Changes run in Auto where the runtime can (0.451): each column is in its own copy.
          routes: comparePicks.map((pick) => {
            // A column's own effort (0.490), turned into what the runtime takes: Cursor's is in the id.
            // The level its chip SHOWED (0.493): the one chosen, else the one its id names, else the
            // model's default. Sol's 0.491 pass: Medium, a default, went unnamed beside a chosen High.
            const family = modelFamily(models, pick.runtime, pick.model)
            const levels = family?.supportedEfforts ?? []
            const idLevel = Object.entries(family?.variants ?? {}).find(([, id]) => id === pick.model)?.[0]
            const level = levels.length === 0 ? undefined : pick.effort ?? idLevel ?? defaultEffort(levels, family?.defaultEffort)
            const started = startRoute(models, pick.runtime, pick.model, level)
            return {
              runtime: pick.runtime, ...(pick.account === undefined ? {} : { account: pick.account }),
              model: started.model,
              ...(started.effort === undefined ? {} : { effort: started.effort }),
              // Every column is named with its level, so two levels of one model read apart.
              label: level === undefined ? pick.label : `${pick.label} · ${effortName(level)}`,
              ...(compareChanges && modeRunsOn('auto', pick.runtime, build?.platform) ? { mode: 'auto' as const } : {})
            }
          }),
          ...(compareChanges ? { changes: true } : {}),
          ...(compareBlind ? { blind: true } : {})
        })
        .catch(() => undefined)
      if (answer === undefined) return 'The comparison could not be started. Nothing was sent.'
      if (!answer.ok) return answer.error.message
      replaceCompare(answer.data.compare)
      setCompareProblem(undefined)
      setComparingId(answer.data.compare.compareId)
      setCompareOn(false)
      setComparePicks([])
      setCompareChanges(false)
      setCompareBlind(false)
      return true
    }
    return startFromComposer(prompt, sendOptions?.leaveOut, undefined, sendOptions?.referenceIds)
  }

  /** Keep one column: the others stop, and the conversation carries on from the kept one. */
  const keepCompareColumn = async (compare: PublicCompare, slot: CompareSlotId): Promise<void> => {
    const bridge = window.desktop
    if (!bridge) return
    setKeepingCompare(true)
    setCompareProblem(undefined)
    const modeBeforeKeep = mode
    try {
      for (const column of compare.slots) {
        if (column.slot === slot) continue
        for (const [runKey, run] of runs.entries()) {
          if (runKey.startsWith('pending:') || !liveRunIsActive(run)) continue
          if (run.data?.missionId !== undefined && column.missionIds.includes(run.data.missionId)) {
            await bridge.cancelCodexMission({ runId: runKey }).catch(() => undefined)
          }
        }
      }
      const answer = await bridge.keepCompare(compare.compareId, slot).catch(() => undefined)
      if (answer === undefined || !answer.ok) {
        setCompareProblem(answer?.ok === false ? answer.error.message : 'That column could not be kept. Nothing changed.')
        return
      }
      replaceCompare(answer.data.compare)
      const kept = answer.data.compare.slots.find((column) => column.slot === slot)
      const newest = kept?.missionIds.at(-1)
      if (newest !== undefined) openMission(newest)
      /*
       * KEPT MEANS CARRIED ON WITH (0.515). A pass on 0.512 kept Longcat's
       * answer: the header said Longcat, the chat box said the teammate's
       * Mimo, and the next message went to Mimo as "a fresh session". The
       * kept column's model is the one the conversation now continues on.
       */
      if (kept !== undefined) {
        const carried = routeAfterKeep(kept.route, models)
        changeRoute({ runtime: carried.runtime, model: carried.model })
        /*
         * ...AT ITS OWN EFFORT (0.612). Arena round 2's reveal, 2026-10-04: "You
         * kept Sonnet 5.5 · High; the conversation carries on with it", and the
         * chat box beneath it said Medium. changeRoute carries the CHAT BOX's
         * effort over to a new model, as a person's pick should; here the
         * effort that ran is the kept column's, so that is the one carried, by
         * the same rule (the model's default when it does not offer it).
         */
        setEffort(carried.effort)
        /*
         * ...BUT NOT ITS MODE (0.622). A compare's Auto worked in copies, and
         * the conversation goes on in the folder itself, at the chat box's own
         * mode. Opening the kept mission reads a mode back from its record
         * (followRouteOf), and for a column that ran in Auto that is Auto: the
         * real folder, switched silently. So the mode the person had when they
         * pressed Keep is put back, and the bar says which one it is.
         */
        setMode(modeBeforeKeep)
      }
      setComparingId(undefined)
    } finally {
      setKeepingCompare(false)
    }
  }

  /** Ask one column again, on the same model (0.444): a provider that was down gets another go. */
  /** Ask a judge for its view of the answers (0.520): one read-only run on the model picked. */
  const judgeCompare = async (compare: PublicCompare, choice: { readonly runtime: MissionRuntimeId; readonly model: string; readonly label: string; readonly account?: string }, criteria: string, panel: readonly { readonly runtime: MissionRuntimeId; readonly model: string; readonly label: string; readonly account?: string }[] = []): Promise<void> => {
    const bridge = window.desktop
    if (!bridge) return
    setJudgingCompare(true)
    setCompareProblem(undefined)
    try {
      const answer = await bridge.judgeCompare({
        compareId: compare.compareId,
        route: { runtime: choice.runtime, model: choice.model, ...(choice.account === undefined ? {} : { account: choice.account }), label: choice.label },
        ...(criteria.trim().length === 0 ? {} : { criteria }),
        ...(panel.length === 0 ? {} : { panel: panel.map((one) => ({ runtime: one.runtime, model: one.model, ...(one.account === undefined ? {} : { account: one.account }), label: one.label })) })
      }).catch(() => undefined)
      if (answer === undefined || !answer.ok) {
        setCompareProblem(answer?.ok === false ? answer.error.message : 'The judge could not be asked. Nothing was started.')
        return
      }
      replaceCompare(answer.data.compare)
    } finally {
      setJudgingCompare(false)
    }
  }
  /** Put the comparison's question to one more model, as a new column (0.523). */
  const addCompareModel = async (compare: PublicCompare, choice: { readonly runtime: MissionRuntimeId; readonly model: string; readonly label: string; readonly account?: string }): Promise<void> => {
    const bridge = window.desktop
    if (!bridge) return
    setAddingCompareModel(true)
    setCompareProblem(undefined)
    try {
      // The column is named by its model, as the others are; its runtime's mark sits beside it.
      const label = routeModelName(choice.runtime, choice.model, resolvedModels.get(`${choice.runtime}:${choice.model}`))
      const answer = await bridge.addCompareModel({ compareId: compare.compareId, route: { runtime: choice.runtime, model: choice.model, ...(choice.account === undefined ? {} : { account: choice.account }), label } }).catch(() => undefined)
      if (answer === undefined || !answer.ok) {
        setCompareProblem(answer?.ok === false ? answer.error.message : 'That model could not be asked. Nothing was started.')
        return
      }
      replaceCompare(answer.data.compare)
    } finally {
      setAddingCompareModel(false)
    }
  }
  const retryCompareColumn = async (compare: PublicCompare, slot: CompareSlotId): Promise<void> => {
    const bridge = window.desktop
    if (!bridge) return
    setRetryingCompare(slot)
    setCompareProblem(undefined)
    try {
      const answer = await bridge.retryCompare(compare.compareId, slot).catch(() => undefined)
      if (answer === undefined || !answer.ok) {
        setCompareProblem(answer?.ok === false ? answer.error.message : 'That column could not be asked again. Nothing was started.')
        return
      }
      replaceCompare(answer.data.compare)
    } finally {
      setRetryingCompare(undefined)
    }
  }

  /** A comparison run again, once or three times more (0.763): what went wrong, said in its block. */
  const rerunCompare = async (origin: PublicCompare, times: number): Promise<void> => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setStartingRerun(true)
    setBenchmarkProblem(undefined)
    try {
      const answer = await bridge.rerunCompare(origin.compareId, times).catch(() => undefined)
      if (answer === undefined) setBenchmarkProblem('Locust could not start the runs. Nothing was started.')
      else if (!answer.ok) setBenchmarkProblem(answer.message)
    } finally {
      setStartingRerun(false)
    }
  }
  /** A row's model, set as its teammate's own (0.763): every other part of the teammate kept as it is. */
  const useBenchmarkModel = async (teammate: PublicTeammate, row: BenchmarkRow): Promise<void> => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setUsingBenchmarkRow(row.key)
    setBenchmarkProblem(undefined)
    try {
      const answer = await bridge.updateTeammate({
        teammateId: teammate.teammateId,
        name: teammate.name,
        hue: teammate.hue,
        role: teammate.role,
        ...(teammate.roleTitle === undefined ? {} : { roleTitle: teammate.roleTitle }),
        ...(teammate.worktree === undefined ? {} : { worktree: teammate.worktree }),
        avatar: teammate.avatar,
        route: { runtime: row.runtime, model: row.model, mode: teammate.route?.mode ?? 'accept-edits', ...(row.effort === undefined ? {} : { effort: row.effort }) }
      }).catch(() => undefined)
      if (answer === undefined || !answer.ok) {
        setBenchmarkProblem(answer?.ok === false ? answer.error.message : `${teammate.name}'s model could not be set. Nothing was changed.`)
        return
      }
      const listed = await bridge.listTeammates().catch(() => undefined)
      if (listed?.ok === true) {
        setTeammates(listed.data.teammates)
        setMissionOwners(listed.data.missionOwners)
        rememberRosterNames(listed.data)
      }
    } finally {
      setUsingBenchmarkRow(undefined)
    }
  }
  /**
   * YOUR OWN BENCHMARK (0.763, benchmark.ts): a comparison its judge has scored, and its runs again, read as one table
   * and a chart. Undefined before its judge has scored it, and for a run again itself: the comparison it runs again
   * shows it.
   */
  const benchmarkOf = (origin: PublicCompare): BenchmarkView | undefined => {
    if (origin.rerunOf !== undefined || origin.judge === undefined) return undefined
    const first = judgeViewOf(origin)
    if ((first?.scorecard === undefined || first.running) && (origin.reruns ?? []).length === 0) return undefined
    const runsOfIt = [origin, ...(origin.reruns ?? []).flatMap((id) => compares.filter((one) => one.compareId === id))]
    const inputs: BenchmarkRunInput[] = []
    const runList: { readonly compareId: string; readonly state: string; readonly done: boolean }[] = []
    for (const run of runsOfIt) {
      const { columns } = compareColumnsFor(run)
      const judge = judgeViewOf(run)
      const scorecard = judge === undefined || judge.running ? undefined : judge.scorecard
      inputs.push({
        compareId: run.compareId,
        answers: run.slots.map((slot, index) => {
          const column = columns[index]
          const verdicts = scorecard?.verdicts[slot.slot]
          const score = verdicts === undefined ? undefined : scoreOf(verdicts, scorecard?.weights)
          const key = `${slot.route.runtime}:${slot.route.model}`
          return {
            key,
            runtime: slot.route.runtime as MissionRuntimeId, ...(slot.route.account === undefined ? {} : { account: slot.route.account }),
            model: slot.route.model,
            ...(slot.route.effort === undefined ? {} : { effort: slot.route.effort }),
            name: slot.route.label ?? routeModelName(slot.route.runtime as MissionRuntimeId, slot.route.model, resolvedModels.get(key)),
            answered: column?.keepable === true,
            ...(score === undefined ? {} : { met: score.met, of: score.of }),
            ...(column?.spanMs === undefined ? {} : { spanMs: column.spanMs }),
            ...(column?.runCost === undefined ? {} : { cost: column.runCost })
          }
        })
      })
      // Not scored, said with why (0.763): a judge reads two finished answers or none, and one may give no scores.
      const answeredHere = columns.filter((column) => column.keepable).length
      const over = run.rerunOf === undefined || run.settled === true
      runList.push({
        compareId: run.compareId,
        state: columns.some((column) => column.running) ? 'answering…'
          : judge?.running === true ? 'judging…'
          : scorecard !== undefined ? 'scored'
          : !over ? 'waiting'
          : answeredHere < 2 ? `not scored · ${String(answeredHere)} of ${String(columns.length)} answered`
          : judge !== undefined ? 'not scored · the judge gave no scores'
          : 'not scored',
        done: over && !columns.some((column) => column.running) && judge?.running !== true
      })
    }
    const rows = benchmarkRows(inputs)
    const chart = benchmarkChart(rows)
    const progress = rerunProgress.find((one) => one.originId === origin.compareId)
    const teammate = origin.teammateId === undefined ? undefined : teammates.find((one) => one.teammateId === origin.teammateId)
    const refusal = rerunRefusalOf(origin)
    return {
      ...(refusal === undefined ? {} : { refusal }),
      ...((origin.judge?.panel ?? []).length > 0 ? { threeJudges: true } : {}),
      rows,
      ...(chart === undefined ? {} : { chart }),
      runs: runList,
      ...(progress === undefined ? {} : { progress }),
      ...(origin.base === undefined ? {} : { base: origin.base }),
      ...(teammate === undefined ? {} : { teammateName: teammate.name, ...(teammate.route === undefined ? {} : { teammateKey: `${teammate.route.runtime}:${teammate.route.model}` }) }),
      busy: startingRerun,
      ...(usingBenchmarkRow === undefined ? {} : { using: usingBenchmarkRow }),
      ...(benchmarkProblem === undefined ? {} : { problem: benchmarkProblem }),
      onRun: (times: number) => void rerunCompare(origin, times),
      onStop: () => void window.desktop?.stopCompareRerun(origin.compareId).catch(() => undefined),
      onOpenRun: (compareId: string) => setComparingId(compareId),
      ...(teammate === undefined ? {} : { onUse: (row: BenchmarkRow) => void useBenchmarkModel(teammate, row) })
    }
  }

  /** YOUR BENCHMARKS (0.764, BenchmarksScreen.tsx): every comparison run again, a row each. */
  const listBenchmarks = (): readonly BenchmarkEntry[] => compares.flatMap((origin) => {
    if (origin.rerunOf !== undefined || (origin.reruns ?? []).length === 0) return []
    const view = benchmarkOf(origin)
    if (view === undefined) return []
    const nameOf = (route: CompareRoute): string => route.label ?? routeModelName(route.runtime as MissionRuntimeId, route.model, resolvedModels.get(`${route.runtime}:${route.model}`))
    const runsOfIt = [origin, ...(origin.reruns ?? []).flatMap((id) => compares.filter((one) => one.compareId === id))]
    const lastAt = runsOfIt.map((run) => run.createdAt).reduce((newest, at) => (Date.parse(at) > Date.parse(newest) ? at : newest))
    const owner = origin.teammateId === undefined ? undefined : teammates.find((one) => one.teammateId === origin.teammateId)
    return [{
      compareId: origin.compareId,
      question: origin.prompt,
      ...(owner === undefined ? {} : { owner }),
      models: origin.slots.map((slot) => nameOf(slot.route)),
      runs: view.runs.length,
      ...(origin.judge === undefined ? {} : { judgeName: nameOf(origin.judge.route) }),
      lastAt,
      rows: view.rows,
      ...(view.progress === undefined ? {} : { progress: view.progress }),
      changes: origin.changes === true
    }]
  })
  /** A benchmark opened from its list: in its teammate's conversation, as the sidebar opens a comparison's row. */
  const openBenchmark = (compareId: string): void => {
    const origin = compares.find((one) => one.compareId === compareId)
    if (origin === undefined) return
    const kept = origin.kept === undefined ? undefined : origin.slots.find((slot) => slot.slot === origin.kept!.slot)
    const missionId = kept?.missionIds.at(-1) ?? origin.slots.find((slot) => slot.missionIds.length > 0)?.missionIds[0]
    if (missionId !== undefined) openMission(missionId)
    else setScreen('workroom')
    setComparingId(compareId)
    setBackToBenchmarks(true)
    setRevealBenchmarkOf(compareId)
  }

  /** The columns, from the live runs where they are going and the record where they are done. */
  /** The judge's view of a comparison, from its newest run (0.520). */
  const judgeViewOf = (compare: PublicCompare): JudgeView | undefined => {
    const judge = compare.judge
    const missionId = judge?.missionIds.at(-1)
    if (judge === undefined || missionId === undefined) return undefined
    /** One judge's run: its whole thread, drawn as every other reply is (0.554), and what it said. */
    const readOf = (id: string): { readonly running: boolean; readonly items: readonly ThreadItem[]; readonly answer: string } => {
      const live = [...runs.values()].find((run) => run.data?.missionId === id)
      const recorded = historyById.get(id)
      const events = live !== undefined && live.events.length > 0 ? live.events : recorded?.events ?? []
      const running = (live !== undefined && liveRunIsActive(live)) || (live === undefined && recorded === undefined)
      const items = buildThread(events, { running, trimmed: live !== undefined && live.events.length > 0 ? live.eventsTruncated === true : recorded?.eventsTruncated === true, mayEdit: false, ...(workspacePath === undefined ? {} : { workspacePath }) })
      const answer = items
        .flatMap((item) => (item.type === 'agent-message' && item.text.trim().length > 0 ? [item.text.trim()] : []))
        .join('\n\n')
      return { running, items, answer }
    }
    const nameOf = (route: CompareRoute): string => route.label ?? routeModelName(route.runtime as MissionRuntimeId, route.model, resolvedModels.get(`${route.runtime}:${route.model}`))
    // Its words without its scorecard lines, which the table says.
    const saidOf = (items: readonly ThreadItem[]): readonly ThreadItem[] => items.flatMap((item): ThreadItem[] => {
      if (item.type !== 'agent-message') return [item]
      const text = withoutScorecard(item.text)
      return text.length === 0 ? [] : [{ ...item, text }]
    })
    const slots = compare.slots.map((column) => column.slot)
    const name = nameOf(judge.route)
    const first = readOf(missionId)
    if ((judge.panel ?? []).length === 0) {
      const { running, items, answer } = first
      if (!running && answer.length === 0) return { name, running, answer, items, failed: 'The judge did not answer. Ask again, or pick another model.' }
      const pick = running ? undefined : judgePickOf(answer, slots)
      // Its scorecard (0.759): read from what it said once it is done.
      const scorecard = running ? undefined : weighted(parseScorecard(answer, slots, criteriaLines(judge.criteria)), criteriaWeights(judge.criteria))
      return { name, running, answer, items: saidOf(items), ...(pick === undefined ? {} : { pick }), ...(scorecard === undefined ? {} : { scorecard }) }
    }
    /*
     * THREE JUDGES VOTING (0.765, compare-score.ts panelVote): each read the answers on its own, on the person's
     * points. The scorecard is their vote, once all three are done; the words drawn are the first one's that said any,
     * and a judge that did not answer has no vote.
     */
    // Each by its model, not "Codex CLI / GPT-6.1-Sol": three of those ran the heading onto two lines (the 0.765
    // drive's frame). An account's default is its agent's name.
    const shortOf = (route: CompareRoute): string => (route.model === 'account-default' ? runtimeNameOf(route.runtime) : routeModelName(route.runtime as MissionRuntimeId, route.model, resolvedModels.get(`${route.runtime}:${route.model}`)))
    const all = [{ name: shortOf(judge.route), ...first }, ...(judge.panel ?? []).map((member) => ({ name: shortOf(member.route), ...readOf(member.missionId) }))]
    const running = all.some((one) => one.running)
    const answered = all.filter((one) => !one.running && one.answer.length > 0)
    const shown = all.find((one) => one.answer.length > 0) ?? all[0]!
    const judges = all.map((one) => one.name)
    const silent = running ? [] : all.filter((one) => one.answer.length === 0).map((one) => one.name)
    const panel = { judges, wordsOf: shown.name, silent }
    const together = joinedNames(judges)
    if (!running && answered.length === 0) return { name: together, running, answer: '', items: first.items, panel, failed: 'None of the judges answered. Ask again, or pick other models.' }
    const points = criteriaLines(judge.criteria)
    const scorecard = running ? undefined : weighted(panelVote(answered.map((one) => parseScorecard(one.answer, slots, points))), criteriaWeights(judge.criteria))
    const pick = running ? undefined : panelPick(answered.map((one) => judgePickOf(one.answer, slots)), answered.length)
    return { name: together, running, answer: shown.answer, items: saidOf(shown.items), panel, ...(pick === undefined ? {} : { pick }), ...(scorecard === undefined ? {} : { scorecard }) }
  }
  /**
   * The models a judge can be (0.520): ready ones, a free model that has
   * answered here first, then the chat box's, then the catalogue -- never
   * one of the compared models while another can judge (Optima's judges are
   * not the contestants).
   */
  const judgeChoicesFor = (compare: PublicCompare): readonly { readonly key: string; readonly label: string; readonly runtime: MissionRuntimeId; readonly model: string; readonly group: string; readonly short: string }[] => {
    const compared = new Set(compare.slots.map((column) => `${column.route.runtime}:${column.route.model}`))
    const ready = (runtime: string): boolean => runtimes.some((status) => status.id === runtime && status.ready) && compareRefusalOf(runtime, antigravityCli) === undefined
    const picked: { key: string; label: string; runtime: MissionRuntimeId; model: string; group: string; short: string }[] = []
    const add = (runtime: MissionRuntimeId, model: string): void => {
      const key = `${runtime}:${model}`
      if (picked.some((one) => one.key === key) || !ready(runtime)) return
      const short = routeModelName(runtime, model, resolvedModels.get(key))
      picked.push({ key, runtime, model, label: `${runtimeNameOf(runtime)} / ${short}`, group: runtimeNameOf(runtime), short })
    }
    for (const model of freeAnswered) add('opencode', model)
    add(composerRoute.runtime, composerRoute.model)
    const catalogue = models.filter((model) => model.older !== true && model.own !== true)
    for (const model of [...catalogue.filter((one) => one.id.endsWith('-free')), ...catalogue.filter((one) => !one.id.endsWith('-free'))]) add(model.runtime, model.id)
    const others = picked.filter((one) => !compared.has(one.key))
    const ordered = [...others, ...picked.filter((one) => compared.has(one.key))]
    // Every one (0.554): the offered one first, the rest under their agent in the catalogue's order.
    const [offered, ...rest] = ordered
    const runtimeOrder = [composerRoute.runtime, ...catalogue.map((model) => model.runtime)]
    const rank = (one: { readonly runtime: string }): number => runtimeOrder.indexOf(one.runtime as MissionRuntimeId)
    return offered === undefined ? [] : [offered, ...rest.map((one, index) => ({ one, index })).sort((x, y) => rank(x.one) - rank(y.one) || x.index - y.index).map(({ one }) => one)]
  }
  const compareColumnsFor = (compare: PublicCompare): { readonly prompts: readonly string[]; readonly columns: readonly CompareColumnView[] } => {
    // Each column's files are read where it works (0.555): its copy, until one is kept.
    const cellOf = (missionId: string, folder: string | undefined = workspacePath) => {
      const live = [...runs.values()].find((run) => run.data?.missionId === missionId)
      const recorded = historyById.get(missionId)
      const events = live !== undefined && live.events.length > 0 ? live.events : recorded?.events ?? []
      const running = live !== undefined && liveRunIsActive(live)
      return {
        missionId,
        events,
        running,
        phase: live?.phase ?? recorded?.phase ?? 'unknown',
        prompt: splitAttachments(live?.prompt ?? recorded?.prompt ?? '').text,
        items: buildThread(events, { running, trimmed: live !== undefined && live.events.length > 0 ? live.eventsTruncated === true : recorded?.eventsTruncated === true, mayEdit: false, ...(folder === undefined ? {} : { workspacePath: folder }) })
      }
    }
    const cellsBySlot = compare.slots.map((column) => column.missionIds.map((missionId) => cellOf(missionId, column.folder ?? workspacePath)))
    const columns = compare.slots.map((column, index): CompareColumnView => {
      const cells = cellsBySlot[index] ?? []
      const running = cells.some((cell) => cell.running)
      const last = cells.at(-1)
      const spanMs = cells.reduce((total, cell) => total + (runSpanMs(cell.events) ?? 0), 0)
      const runCost = sumCosts(cells.map((cell) => runCostOf(cell.events)))
      const cost = costLine(runCost)
      const state =
        cells.length === 0 ? (column.refused === undefined ? 'waiting' : 'could not start')
        : running ? 'working'
        : last?.phase === 'completed' ? 'done'
        : last?.phase === 'cancelled' ? 'stopped'
        : last?.phase === 'failed' ? 'failed'
        : String(last?.phase ?? '')
      return {
        slot: column.slot,
        // Blind until one is kept (0.449): Model A, Model B.
        name: compare.blind === true && compare.kept === undefined ? blindName(column.slot, hasColumnInstructions(compare)) : column.route.label ?? column.route.model,
        runtime: column.route.runtime as MissionRuntimeId,
        runtimeName: runtimeNameOf(column.route.runtime),
        ...(compareNeedsCopy(column.route.runtime, build?.platform) ? { copy: true } : {}),
        ...(column.refused === undefined ? {} : { refused: column.refused }),
        ...(column.folder === undefined ? {} : { folder: column.folder }),
        turns: cells.map((cell) => ({ missionId: cell.missionId, items: cell.items, running: cell.running })),
        approvals: approvals.filter((request) => {
          const missionId = runs.get(request.runId)?.data?.missionId
          return missionId !== undefined && column.missionIds.includes(missionId)
        }),
        running,
        keepable: !running && last?.phase === 'completed',
        // Not for a folder too big to copy (R9): asking again gives the same answer.
        retryable: compare.kept === undefined && !running && !/too big to copy/.test(column.refused ?? '') && (last === undefined ? column.refused !== undefined : last.phase !== 'completed'),
        answer: (last?.items ?? [])
          .flatMap((item) => (item.type === 'agent-message' && item.text.trim().length > 0 ? [item.text.trim()] : []))
          .join('\n\n'),
        state,
        ...(spanMs > 0 ? { span: durationText(spanMs), spanMs } : {}),
        ...(cost === undefined ? {} : { cost }),
        ...(runCost === undefined ? {} : { runCost })
      }
    })
    // Each ask once, from the column that has taken the most of them.
    const longest = cellsBySlot.reduce((best, cells) => (cells.length > best.length ? cells : best), [] as ReturnType<typeof cellOf>[])
    const prompts = longest.length === 0 ? [compare.prompt] : longest.map((cell, turn) => (turn === 0 ? compare.prompt : cell.prompt))
    return { prompts, columns }
  }

  /**
   * Send a message to the teammates tagged in it (0.438, shared/tagging.ts),
   * each in a conversation of their own; the conversation on screen is where
   * it came from. Answers the line the composer shows.
   */
  const tagTeammates = async (teammateIds: readonly string[], prompt: string): Promise<{ readonly text: string; readonly sent: boolean } | undefined> => {
    const bridge = window.desktop
    if (!bridge) return { text: 'Locust is not ready yet. Nobody tagged was sent it.', sent: false }
    const fromMissionId = liveRunRef.current?.data?.missionId
    try {
      const answer = await bridge.tagTeammates({
        teammateIds,
        message: prompt,
        ...(pickedTeammate === undefined ? {} : { fromTeammateId: pickedTeammate.teammateId }),
        ...(fromMissionId === undefined ? {} : { fromMissionId })
      })
      if (!answer.ok) return { text: answer.message, sent: false }
      const sent = answer.started.length === 0 ? '' : `Sent to ${answer.started.join(' and ')}${pickedTeammate === undefined ? '' : ' too'}.`
      const refused = answer.refused.map((entry) => entry.message.startsWith(entry.name) ? entry.message : `${entry.name}: ${entry.message}`).join(' ')
      const text = [sent, refused].filter((part) => part.length > 0).join(' ')
      // Plain only when everyone tagged was sent it; a refusal keeps the warning's look.
      return text.length === 0 ? undefined : { text, sent: refused.length === 0 }
    } catch {
      return { text: 'The tagged teammates could not be sent it.', sent: false }
    }
  }
  const startFromComposer = async (prompt: string, leaveOut?: readonly string[], cloud?: boolean, referenceIds?: readonly string[]): Promise<boolean | string> => {
    startRefusal.current = undefined
    const rewind = rewinding !== undefined && rewinding.key === shownKey ? rewinding : undefined
    // Files first, so the teammate starts on the folder as it was (0.502).
    const putBack = rewind !== undefined && rewind.putBack && rewind.files.length > 0
      ? await window.desktop?.putBackFiles({ files: rewind.files }).catch(() => undefined)
      : undefined
    if (rewind !== undefined && rewind.putBack && rewind.files.length > 0 && putBack === undefined) {
      return 'Not sent: the files could not be put back, so nothing was changed. Untick it to send the edit on its own.'
    }
    const started = await startMission(prompt, undefined, { fromComposer: true, ...(referenceIds === undefined ? {} : { referenceIds }), ...(cloud === true ? { cloud: true } : {}), ...(leaveOut === undefined || leaveOut.length === 0 ? {} : { leaveOut }), ...(rewind === undefined ? {} : { rewind: { ...rewind, ...(putBack === undefined ? {} : { putBackResult: putBack }) } }) })
    if (started && rewind !== undefined) setRewinding(undefined)
    return started ? true : (startRefusal.current ?? false)
  }
  // Another conversation opened: the edit was of that one, and is let go.
  useEffect(() => {
    if (rewinding !== undefined && rewinding.key !== shownKey) setRewinding(undefined)
  }, [shownKey, rewinding])
  /** Edit on a sent message (0.498): its words back in the box, and the next send branches from before it. */
  const editEarlierMessage = (missionId: string, words: string): void => {
    const shown = liveRunRef.current
    if (shown?.data === undefined || shown.phase === 'running' || shown.phase === 'starting') return
    const turns = [
      ...(shown.earlierTurns ?? []),
      { missionId: shown.data.missionId, prompt: shown.prompt, events: shown.events }
    ]
    const index = turns.findIndex((turn) => turn.missionId === missionId)
    if (index < 0 || shownKey === undefined) return
    const before = turns[index - 1]?.missionId
    setRewinding({
      key: shownKey,
      tip: shown.data.missionId,
      ...(before === undefined ? {} : { before }),
      earlierTurns: turns.slice(0, index),
      files: laterFileChanges(turns.slice(index), workspacePath),
      // The turns the edit sets aside: what they sent and nobody read yet is never delivered (0.512).
      setAside: turns.slice(index).map((turn) => turn.missionId),
      putBack: false
    })
    setComposerFill((current) => ({ text: words, seq: (current?.seq ?? 0) + 1 }))
  }
  const claimCommands = (prompt: string, key: string): string => {
    const pending = commandsStillToCarry(commandEntriesRef.current.filter((entry) => entry.key === key))
    if (pending.length === 0) return prompt
    const ids = new Set(pending.map((entry) => entry.id))
    const next = commandEntriesRef.current.map((entry) => ids.has(entry.id) ? { ...entry, carried: true } : entry)
    commandEntriesRef.current = next
    setCommandEntries(next)
    return promptWithCommands(prompt, pending)
  }
  const startMission = async (
    prompt: string,
    modeOverride?: MissionMode,
    /** Sent from the queue: refused as busy, it goes back in line (retriedAfterBusy). */
    options?: {
      readonly requeue?: QueuedRow
      /**
       * Started FOR a teammate who is not on screen yet -- a review, a tidy
       * pass (H9). They are carried whole, and the run is a NEW conversation
       * of theirs: whatever is on screen is not theirs to continue.
       */
      readonly as?: StartAs
      /** Typed in the box and sent from it: a refusal can hand the words back. */
      readonly fromComposer?: boolean
      readonly referenceIds?: readonly string[]
      /** Sent with Cloud for a teammate whose maker has no cloud: it runs on the person's cloud computer (0.761). */
      readonly cloud?: boolean
      /** A reply on another runtime: the brief sections the person left out (0.527). */
      readonly leaveOut?: readonly string[]
      /** Started again from an edited earlier message (0.498). */
      readonly rewind?: { readonly tip: string; readonly before?: string; readonly earlierTurns: NonNullable<LiveRunState['earlierTurns']>; readonly putBackResult?: RewindPutBackResponse; readonly setAside?: readonly string[] }
    }
  ): Promise<boolean> => {
    const carryKey = options?.fromComposer === true ? draftKeyRef.current : options?.requeue?.origin === 'person' ? options.requeue.key : undefined
    const pendingIssue = options?.fromComposer === true ? pendingIssueRef.current : options?.requeue?.issue
    if (options?.fromComposer === true) pendingIssueRef.current = undefined
    const issue = pendingIssue !== undefined && (options?.requeue !== undefined || prompt.includes(`GitHub issue #${String(pendingIssue.number)}`)) ? pendingIssue : undefined
    if (carryKey !== undefined) prompt = claimCommands(prompt, carryKey)
    const as = options?.as
    const route = as?.route ?? composerRoute
    const runMode = as?.mode ?? mode
    const runEffort = as === undefined ? effort : as.effort
    // What was on screen before this turn's own row took its place.
    const previousKey = shownKey
    const bridge = window.desktop
    const teammateId = as?.teammateId ?? pickedTeammate?.teammateId
    // No folder, no run. The main process refuses this too; saying it here
    // keeps a refused start from being filed as a failed mission.
    if (workspacePath === undefined && build !== undefined) {
      setWorkspaceNotice(
        'Choose the folder your teammates work in first. Locust was opened from its own install folder, and no teammate should work in there.'
      )
      return false
    }
    const key = `pending:${++pendingKeyCounter.current}`
    // A reply continues the conversation on screen, when there IS one to
    // continue: the same teammate's finished mission, on the route it ran on,
    // that actually left a runtime session behind. Anything else is a new
    // mission -- which is what a person means when they switch teammate or
    // route first, and the only thing that can work after a run that failed
    // before its runtime ever started.
    const shown = liveRunRef.current
    // Being the next turn of a conversation and resuming a runtime's session
    // are two different things, and gating the first on the second is what
    // made a reply after a failed run open a SECOND sidebar row and lose the
    // turn before it from the screen (Colin, 2026-09-03: "the old bug where
    // it starts new missions"). A run that failed before its runtime got
    // going left no session to resume -- it did not stop being the turn the
    // person was replying to. So the conversation continues either way, and
    // whether the model gets the earlier messages is asked separately.
    // The route no longer has to match: a reply on another runtime is a
    // checkpointed continuation now (the host briefs the new runtime from the
    // old run's record), so it is still the same conversation on screen. The
    // 0.21.2 QA pass had to start a stranger and retype the task instead.
    /*
     * Whether the run is still LIVE is not this decision to make.
     *
     * It was, and it cost the thread: "Build this plan" clicked while the plan
     * run was still settling recorded the build turn with no `continuesFrom`
     * at all, so it started a stranger instead of following the plan it was
     * offered from. Intermittent by construction -- terminal events land once
     * per frame, so for a few tens of milliseconds the host has released the
     * mission and this has not heard (`docs/FINDING-smoke-sweep-2026-09-11.md`,
     * the `steering` smoke: PASS, FAIL, FAIL, PASS).
     *
     * The host settles it, and settles it STRICTER: `ownerBusy` is checked
     * and returns before `followUpOf` is so much as read, so a follow-up on a
     * genuinely running mission cannot be honoured -- the start is refused
     * first, and a refusal now queues the message and asks again when the run
     * in front of it ends. There is no window left in which this is wrong.
     */
    const rewind = options?.rewind
    // A rewind continues from the turn BEFORE the edited one, which the host resolves (0.498).
    const continuing =
      rewind === undefined && as === undefined && shown !== undefined && shown.data !== undefined && ownerOf(shown) === teammateId ? shown : undefined
    const coldStart = continuing !== undefined && resumableSessionOf(continuing.events) === undefined
    const earlierTurns = rewind !== undefined ? rewind.earlierTurns : continuing === undefined
      ? []
      : [
          ...(continuing.earlierTurns ?? []),
          {
            missionId: continuing.data!.missionId,
            prompt: continuing.prompt,
            events: continuing.events,
            // What that turn exchanged with peers, carried forward with it.
            // Dropping it here is what hid the outgoing half of a teammate
            // exchange even after the thread learned to draw earlier turns:
            // the message Booty sent Wren lived on the turn BEFORE the reply,
            // and this is where that turn was rebuilt without it.
            ...(continuing.peerMessages === undefined ? {} : { peerMessages: continuing.peerMessages }),
            // A turn Locust carried on (0.774) stays Locust's once the person replies under it.
            ...(continuing.startedBy?.kind === 'carry-on' ? { startedBy: continuing.startedBy } : {}),
            // And where that turn itself switched runtime, so its divider
            // stays above it once the conversation moves on.
            ...(continuing.switchedFrom === undefined ? {} : { switchedFrom: continuing.switchedFrom }),
            ...(continuing.versions === undefined ? {} : { versions: continuing.versions }),
            // And whether it was had in the terminal: without it the reply's
            // thread drew "Back in Locust" ABOVE the turn it was replying to
            // (the 0.391 drive's own text, read after its checks had passed).
            ...inTerminalOf(continuing)
          }
        ]
    const starting: LiveRunState = {
      prompt,
      phase: 'starting',
      events: [],
      startedAtIso: new Date().toISOString(),
      runtime: route.runtime, ...(route.account === undefined ? {} : { account: route.account }),
      ...(teammateId === undefined ? {} : { teammateId }),
      ...(earlierTurns.length === 0 ? {} : { earlierTurns }),
      ...(coldStart ? { coldStart: true } : {}),
      ...(rewind === undefined ? {} : { rewound: true, ...(rewind.before === undefined ? {} : { versions: { before: rewind.tip } }) }),
      ...(rewind?.putBackResult === undefined ? {} : { putBack: rewind.putBackResult }),
      // Plan is a mode now, so the run remembers what it was asked to be
      // rather than a switch that sat beside the mode and could disagree.
      ...((modeOverride ?? runMode) === 'plan' ? { plan: true } : {})
    }
    setRuns((current) => withNewRun(current, key, starting))
    setShownKey(key)
    if (!bridge) {
      setRuns((current) =>
        withRun(current, key, (run) => ({ ...run, phase: 'failed', error: 'The secure desktop bridge is unavailable.' }))
      )
      return false
    }

    try {
      const response = await bridge.startCodexMission({
        prompt,
        // The mode the composer SHOWS, which is not always the mode last
        // chosen: a mode the route cannot run is not one a mission can start
        // in, and sending it anyway is how every message came back refused.
        mode: modeRunsOn(modeOverride ?? runMode, route.runtime, build?.platform)
          ? modeOverride ?? runMode
          : modesFor(route.runtime, build?.platform)[0] ?? 'accept-edits',
        runtime: route.runtime, ...(route.account === undefined ? {} : { account: route.account }),
        // The concrete model. When a runtime encodes effort in the id, the
        // chosen effort names a different model, and sending the family's
        // default with an effort beside it would run the wrong one.
        // Only sent when the chosen model advertised it; the composer cannot
        // offer an effort the catalog did not report for that model. Swarm
        // overrides the picked effort with the model's maximum, and the
        // composer shows that -- so what is sent must match what is shown.
        // Where the effort lives inside the model id, it is sent as the id
        // alone (startRoute).
        ...startRoute(models, route.runtime, route.model, swarmEffortFor(models, route.model, swarm, runEffort, route.runtime)),
        modelChoice: route.model,
        // The host keeps a teammate's saved model unless this route was chosen
        // here: picked in this chat, or the conversation's own (0.552).
        ...(teammateId !== undefined && as === undefined && (pickerRoutes.has(pickKey) || conversationRoute !== undefined) ? { routeOverrideFor: teammateId } : {}),
        ...(teammateId === undefined ? {} : { teammateId }),
        ...(options?.referenceIds === undefined ? {} : { referenceIds: options.referenceIds }),
        // A review's Ask is for that run only (0.514): the host leaves the saved route alone.
        ...(as?.oneOff === true ? { keepSavedRoute: true } : {}),
        ...(continuing === undefined ? {} : { followUpOf: continuing.data!.missionId }),
        ...(continuing === undefined || options?.leaveOut === undefined ? {} : { leaveOut: options.leaveOut }),
        ...(options?.cloud === true ? { cloud: true as const } : {}),
        ...(issue === undefined ? {} : { issue }),
        ...(rewind === undefined ? {} : { ...(rewind.before === undefined ? {} : { followUpOf: rewind.before }), rewind: { tip: rewind.tip, ...(rewind.setAside === undefined ? {} : { setAside: rewind.setAside }) } })
      })
      if (!response.ok) {
        /*
         * "Already running" is NOT A FAILURE, and must not eat the message.
         *
         * A teammate takes one mission at a time, so a message sent a moment
         * too early is refused -- and the turn was marked `failed`, drawn in
         * the terminal red register under "The run could not continue", and
         * the composer let go of the text because the turn was on screen. So
         * the person's sentence was gone and the conversation said it had
         * broken. Colin, 2026-09-11, with a screenshot of exactly that: "
         * message broke off then gave me this error", above a sidebar showing
         * the teammate idle.
         *
         * The app already has the right answer for this and was not using it:
         * the queue. The message waits for the run in front of it and goes
         * when that one finishes, which is what a person means by sending it.
         */
        if (response.error.code === 'RUN_ALREADY_ACTIVE') {
          // In front: this conversation's own turn still running (0.551) -- never another conversation's.
          const inFront = continuing?.data === undefined
            ? undefined
            : [...runs.entries()].find(([, run]) => liveRunIsActive(run) && run.data?.missionId === continuing.data!.missionId)?.[0]
          setRuns((current) => {
            const next = new Map(current)
            next.delete(key)
            return next
          })
          // The row just deleted was what the window showed: put back the
          // conversation it replaced, or the window is left on nothing and
          // falls back to the teammate's home screen (0.297 drive, turn 5).
          setShownKey((current) => (current === key ? previousKey : current))
          // Stopped while it was starting (code review B4, renderer-thread
          // (e)): the person pressed Stop on this message, so it is not queued
          // to go later. It was queued, and sent when the run in front ended.
          if (cancelWhenNamedRef.current.delete(key)) {
            startRefusal.current = 'Stopped before it was sent. Your message is back in the box.'
            return false
          }
          // An edited earlier message is never queued (0.500): the queue sends plain replies,
          // and this one would go out at the END of the conversation instead of where it was edited.
          if (rewind !== undefined) {
            startRefusal.current = 'Not sent: this conversation is still answering. Your edited message is still in the box -- send it once it finishes.'
            return false
          }
          if (inFront !== undefined) {
            return await setQueued((rows) => [...rows, { id: `q_${String(rows.length)}_${inFront}`, key: inFront, text: prompt, origin: 'person' as const, ...(issue === undefined ? {} : { issue }) }])
          }
          // Nothing of theirs is on screen to wait behind -- the cap is full,
          // or the run belongs to a window this one cannot see. The words go
          // back in the box, which is the only other honest place for them --
          // or, for a message the queue sent, back in the queue: it went
          // because the run in front ended HERE, and the host can still be
          // finishing that run. Dropped, it was simply gone.
          if (options?.requeue !== undefined) {
            const back = retriedAfterBusy(options.requeue, previousKey ?? options.requeue.key, Date.now())
            return await setQueued((rows) => [back, ...rows])
          }
          startRefusal.current = `Not sent: ${response.error.message} Your message is back in the box.`
          return false
        }
        /*
         * AT THE MONTHLY LIMIT (0.353) nothing started, and nothing broke: the
         * person set a limit and it held. Drawn as a failure, the turn read
         * "The run could not continue" in red under the person's own words
         * (the 0.353 drive's first run). Claude's answer to its own usage
         * limit is the model: the message is not sent, the words stay where
         * they were typed, and the reason is said beside them. A message the
         * QUEUE sent keeps its card instead -- the words are in its bubble,
         * nowhere else, and its Run it again works once the limit is raised.
         */
        if (response.error.code === 'SPEND_LIMIT_REACHED' && options?.fromComposer === true) {
          setRuns((current) => {
            const next = new Map(current)
            next.delete(key)
            return next
          })
          setShownKey((current) => (current === key ? previousKey : current))
          startRefusal.current = `Not sent: ${response.error.message} Your message is back in the box.`
          return false
        }
        /*
         * NO CLOUD COMPUTER FOR THIS REPOSITORY YET (0.761) -- deleted since Cloud was picked, or GitHub's permission
         * taken back. Nothing started: the words go back in the box and the dialog asks, as it did the first time.
         */
        if (response.error.code === 'CLOUD_SETUP_NEEDED') {
          setRuns((current) => {
            const next = new Map(current)
            next.delete(key)
            return next
          })
          setShownKey((current) => (current === key ? previousKey : current))
          const setup = response.error.cloudSetup
          setCloudSetup(setup === undefined || setup.permission === true ? { phase: 'permission', message: response.error.message } : { phase: 'ask', repo: setup.repo })
          startRefusal.current = 'Not sent: your cloud computer is not set up for this repository yet. Your message is back in the box.'
          return false
        }
        // CLAUDE CODE OR CODEX NOT SIGNED IN THERE (0.768): nothing started; the dialog offers the one-click sign-in.
        if (response.error.code === 'CLOUD_SIGN_IN_NEEDED' && response.error.cloudSignIn !== undefined) {
          setRuns((current) => {
            const next = new Map(current)
            next.delete(key)
            return next
          })
          setShownKey((current) => (current === key ? previousKey : current))
          const signIn = response.error.cloudSignIn
          setCloudSetup({ phase: 'sign-in', repo: signIn.repo, computer: signIn.computer, agent: signIn.agent })
          startRefusal.current = `Not sent: ${response.error.message} Your message is back in the box.`
          return false
        }
        /*
         * A FOLDER LIKE HOME OR .claude, NOT YET SAID YES TO (0.790). Nothing started: the words go back in the box,
         * and the box asks, in a card over it -- Choose another folder, or Start here, which sends them (Composer.tsx).
         * In the window, not a Windows dialog: Colin, 2026-10-18, "maybe make it part of the app?".
         */
        if (response.error.code === 'FOLDER_NOT_CONFIRMED') {
          setRuns((current) => {
            const next = new Map(current)
            next.delete(key)
            return next
          })
          setShownKey((current) => (current === key ? previousKey : current))
          setFolderQuestion({ why: response.error.folderCaution ?? workspaceCaution ?? 'a folder that is no place for teammates', key: previousKey })
          return false
        }
        // A ledger that cannot be written is the same failure whether it hits
        // on the FIRST write or the fortieth, and only the second one used to
        // reach the card that explains it. The first -- a ledger folder that
        // is unwritable when the app starts, which is the likeliest way anyone
        // meets this -- arrived as a plain start error and drew the generic
        // "The run could not continue". Measured by
        // `_tools/drive-ledger-failure.mjs`, which blocks the ledger path.
        const persistence = response.error.code === 'PERSISTENCE_FAILED'
        setRuns((current) =>
          withRun(current, key, (run) => ({
            ...run,
            phase: 'failed',
            error: response.error.message,
            ...(persistence ? { errorIsPersistence: true } : {})
          }))
        )
        // The turn IS on screen -- prompt bubble and failure card -- so the
        // composer must let go of it. Holding on left the same sentence in
        // two places and read as though nothing had been sent (Colin,
        // 2026-09-04: "sometimes text stays in box after sending"). The
        // bridge-missing case above still keeps it, because there the turn
        // was never recorded anywhere and the box is the only copy.
        return true
      }

      const runId = response.data.runId
      // The conversation this starts (or continues) keeps the setup it was sent with (0.552).
      // A turn sent with Cloud keeps Cloud on for the conversation's next turn (0.761).
      chatSetups.current.set(response.data.missionId, { mode: modeOverride ?? runMode, effort: runEffort, ...(options?.cloud === true ? { kind: { compare: false, blind: false, cloud: route.runtime === 'opencode', computer: route.runtime !== 'opencode' } } : {}) })
      // The host recorded the owner; mirror it so the sidebar files the
      // mission under the teammate at once rather than after a refresh.
      if (teammateId !== undefined) {
        const missionId = response.data.missionId
        setMissionOwners((current) => ({ ...current, [missionId]: teammateId }))
        // The host records a route only for a teammate who had none (0.552);
        // mirror that, so their first run's route shows at once.
        const kept = { runtime: response.data.runtime, model: response.data.model, ...(routeAccount(response.data) === undefined ? {} : { account: routeAccount(response.data) }), mode: modeOverride ?? runMode }
        setTeammates((current) =>
          current.map((teammate) => (teammate.teammateId === teammateId && teammate.route === undefined ? { ...teammate, route: kept } : teammate))
        )
      }
      // A pick for a new chat with a teammate was for that chat: it is the
      // conversation's route now, and their next new chat starts on their own.
      if (as === undefined && pickKey.startsWith('new:')) {
        const used = pickKey
        setPickerRoutes((current) => {
          if (!current.has(used)) return current
          const next = new Map(current)
          next.delete(used)
          return next
        })
      }
      const queued = pendingUpdatesRef.current.get(runId) ?? []
      pendingUpdatesRef.current.delete(runId)
      setRuns((current) => {
        const switched = response.data.switchedFrom
        let next: LiveRunState = {
          ...starting,
          // The start's last phase stays on the line (0.602): the response
          // says the program exists, not that it has spoken, and the seconds
          // until it does were drawn as a bare "Starting…" (the dev drive,
          // 2026-10-04: the response at 148 ms, the first word at 14.4 s).
          ...carriedStartPhase(current.get(key)),
          runtime: response.data.runtime,
          data: response.data,
          phase: 'running',
          peerMessages: response.data.peerMessages,
          // Whether it started cold is the host's to say (0.496): it resumes a
          // session the window cannot see -- the one a turn stopped before
          // naming its own was resuming.
          ...(response.data.followsUp === undefined || switched !== undefined
            ? {}
            : { coldStart: response.data.followsUp.runtimeThreadId === undefined }),
          ...(switched === undefined
            ? {}
            : {
                switchedFrom: {
                  from: switched.runtime,
                  to: response.data.runtime,
                  ...(switched.model === undefined ? {} : { fromModel: switched.model }),
                  toModel: response.data.model,
                  at: new Date().toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }),
                  unsettledCount: switched.unsettledCount,
                  omittedBriefing: switched.omittedBriefing,
                  ...(switched.leftOutByYou === undefined ? {} : { leftOutByYou: switched.leftOutByYou })
                }
              }),
          ...(response.data.peerDeliveryFailed
            ? { peerNotices: [{ message: 'Messages from teammates could not be read for this mission. Whatever was waiting is still waiting.', level: 'warning' as const }] }
            : {})
        }
        for (const update of queued) next = applyMissionUpdate(next, update)
        return withNewRun(withoutRun(current, key), runId, next)
      })
      // Follow the run under its real key only if the person is still looking
      // at it; they may have moved to another teammate's thread meanwhile.
      setShownKey((current) => (current === key ? runId : current))
      // Stopped while it was still starting: the person pressed the button
      // and meant it, so the run is cancelled the moment it can be named.
      if (cancelWhenNamedRef.current.delete(key)) {
        setRuns((all) => withRun(all, runId, markedCancelling))
        void bridge
          .cancelCodexMission({ runId })
          .then((answer) => {
            if (answer.ok) return
            setRuns((all) =>
              withRun(all, runId, (run) =>
                liveRunIsActive(run) ? { ...run, phase: 'running', error: answer.error.message } : run
              )
            )
          })
          .catch(() => {
            setRuns((all) =>
              withRun(all, runId, (run) =>
                liveRunIsActive(run)
                  ? { ...run, phase: 'running', error: 'The cancellation request could not be delivered. The mission is still running.' }
                  : run
              )
            )
          })
      }
      // And so does anything queued against it. The temporary key was just
      // deleted above, so a message typed while the host was still answering
      // pointed at nothing and was held as "that conversation is no longer
      // open" -- about the conversation on screen.
      setQueued((current) => requeuedRows(current, key, runId))
      return true
    } catch {
      setRuns((current) =>
        withRun(current, key, (run) => ({ ...run, phase: 'failed', error: 'It could not be started. Nothing was run and nothing was changed.' }))
      )
      return false
    }
  }

  /**
   * Move the shown run to another runtime.
   *
   * The host stops it, reconciles it, and starts a NEW mission briefed from
   * that checkpoint -- so what comes back is a different runId and missionId,
   * and the renderer carries the old run's events forward itself if the
   * thread is to keep reading as one piece of work.
   *
   * Everything about a refusal is surfaced verbatim, because every refusal
   * path in the host describes a mission that is now STOPPED. Swallowing one
   * would leave a dead run looking live.
   */
  /**
   * Pick a stopped mission back up. It reaches the same update stream a start
   * does, so the new run appears and follows exactly as any other; what is
   * different is only that the host wrote its prompt from the checkpoint.
   */
  const resumeMission = async (missionId: string, _epoch: number): Promise<void> => {
    const route = composerRoute
    const bridge = window.desktop
    if (!bridge) return
    setResumeRefusal(undefined)
    setHandingOff(true)
    try {
      // The same route the composer would start a fresh mission on, folded
      // the same way. This used to send the model and NOTHING else, so
      // resuming from a checkpoint silently dropped the effort the chip was
      // showing -- and on Cursor, where effort is encoded in the model id
      // rather than passed beside it, it also ran a different model than the
      // one on screen. The contract already carried `effort`; the renderer
      // was the half that never filled it in.
      const resumed = startRoute(
        models,
        route.runtime,
        route.model,
        swarmEffortFor(models, route.model, swarm, effort, route.runtime)
      )
      const response = await bridge.resumeMission({
        missionId,
        runtime: route.runtime, ...(route.account === undefined ? {} : { account: route.account }),
        // M26: the mode this runtime can run, as a start sends it.
        mode: modeRunsOn(mode, route.runtime, build?.platform) ? mode : modesFor(route.runtime, build?.platform)[0] ?? 'accept-edits',
        ...(resumed.model === 'account-default' ? {} : { model: resumed.model }),
        ...(resumed.effort === undefined ? {} : { effort: resumed.effort })
      })
      if (!response.ok) {
        // M28: said ON THE CARD that was pressed. It went into a run under a
        // key nothing ever showed or listed, so the button just seemed dead.
        setResumeRefusal({ missionId, message: response.error.message })
        return
      }
      const ownerId = missionOwners[missionId]
      if (ownerId !== undefined) {
        setMissionOwners((current) => ({ ...current, [response.data.missionId]: ownerId }))
      }
      // M28: what the host sent before its answer, held for a run the window
      // did not know yet, is this run's -- replayed as a start's is. It was
      // dropped, and a run that finished that fast stayed "running" forever.
      const early = pendingUpdatesRef.current.get(response.data.runId) ?? []
      pendingUpdatesRef.current.delete(response.data.runId)
      setRuns((current) =>
        withNewRun(
          current,
          response.data.runId,
          early.reduce(applyMissionUpdate, {
            prompt: 'Resume from checkpoint',
            data: response.data,
            phase: 'running',
            events: [],
            startedAtIso: new Date().toISOString(),
            ...(ownerId === undefined ? {} : { teammateId: ownerId })
          } as LiveRunState)
        )
      )
      setShownKey(response.data.runId)
      await refreshHistory()
    } finally {
      setHandingOff(false)
    }
  }

  /** Hand the running mission over; the reason, when it was refused with nothing stopped (M9). */
  const handOffMission = async (choice: RouteChoice): Promise<string | undefined> => {
    const bridge = window.desktop
    const current = liveRunRef.current
    const runId = current?.data?.runId
    if (!bridge || current === undefined || runId === undefined || !liveRunIsActive(current)) return undefined

    const from = current.data?.runtime ?? route.runtime
    const fromModel = current.data?.model
    const priorEvents = current.events
    setHandingOff(true)
    const phaseBefore = current.phase
    setRuns((all) => withRun(all, runId, (run) => ({ ...run, phase: 'cancelling', error: undefined })))

    try {
      const response = await bridge.handOffMission({
        runId,
        runtime: choice.runtime, ...(choice.account === undefined ? {} : { account: choice.account }),
        // M26: the mode as chosen. Never widened to one the new runtime can
        // run: Ask onto Cursor on Windows would have become Edit unasked. A
        // mode the runtime cannot run is refused with nothing stopped (M9).
        mode,
        ...startRoute(models, choice.runtime, choice.model, swarmEffortFor(models, choice.model, swarm, effort, choice.runtime))
      })

      if (!response.ok && response.error.untouched === true) {
        // M9: refused before anything was stopped. The run goes on as it
        // was, its questions still asked, and the reason goes to the chat box.
        setRuns((all) => withRun(all, runId, (run) => (run.phase === 'cancelling' ? { ...run, phase: phaseBefore } : run)))
        return response.error.message
      }
      // The new run cannot inherit questions asked of the old one.
      setApprovals((all) => all.filter((entry) => entry.runId !== runId))
      if (!response.ok) {
        // Failed, not cancelled: the mission is over and the reason has to be
        // the thing on screen.
        setRuns((all) => withRun(all, runId, (run) => ({ ...run, phase: 'failed', error: response.error.message })))
        return undefined
      }

      const newRunId = response.data.runId
      setRoute(choice)
      const ownerId = ownerOf(current)
      if (ownerId !== undefined) {
        // M29: the chip follows the route handed TO. Since 0.552 that is the
        // conversation's own route -- the new run's record -- which the box
        // reads before any teammate's; the teammate's saved model is unchanged.
        const missionId = response.data.missionId
        setMissionOwners((owners) => ({ ...owners, [missionId]: ownerId }))
        // Mirrored only for a teammate with no route yet, as the host records it.
        const kept = { runtime: response.data.runtime, model: response.data.model, ...(routeAccount(response.data) === undefined ? {} : { account: routeAccount(response.data) }), mode }
        setTeammates((all) => all.map((teammate) => (teammate.teammateId === ownerId && teammate.route === undefined ? { ...teammate, route: kept } : teammate)))
      }
      const queued = pendingUpdatesRef.current.get(newRunId) ?? []
      pendingUpdatesRef.current.delete(newRunId)
      setRuns((all) => {
        let next: LiveRunState = {
          // The ORIGINAL words, not the generated briefing: the person never
          // typed the briefing, so it must not appear as something they said.
          prompt: current.prompt,
          data: response.data,
          phase: 'running',
          events: [],
          ...(ownerId === undefined ? {} : { teammateId: ownerId }),
          peerMessages: response.data.peerMessages,
          handoff: {
            from,
            to: response.data.runtime,
            ...(fromModel === undefined ? {} : { fromModel }),
            toModel: response.data.model,
            at: new Date().toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }),
            unsettledCount: response.data.unsettledCount,
            omittedBriefing: response.data.omittedBriefing,
            priorEvents
          }
        }
        for (const update of queued) next = applyMissionUpdate(next, update)
        // The stopped run's own terminal receipt still arrives under its old
        // runId; it stays in the map so the sidebar shows both missions, which
        // is what the durable record holds.
        return withNewRun(all, newRunId, next)
      })
      setShownKey((shown) => (shown === runId ? newRunId : shown))
      // A handoff continues the same conversation under another runtime, so
      // the next thing the person said belongs to it and not to the run that
      // was handed off.
      setQueued((current) => requeuedRows(current, runId, newRunId))
      return undefined
    } catch {
      setRuns((all) =>
        withRun(all, runId, (run) => ({ ...run, phase: 'failed', error: 'The handoff request could not be delivered. It stayed on the AI agent it was on.' }))
      )
    } finally {
      setHandingOff(false)
    }
    return undefined
  }

  const cancelMission = (): void => {
    const bridge = window.desktop
    const press = stopPress(liveRun, shownKey)
    if (!bridge || press.kind === 'nothing') return
    // Pressed before the host has answered with a run id. The runtime is
    // already going, so this is not a press to ignore -- it is remembered
    // against the key the run has right now, and the start honours it as
    // soon as there is something to name. Silently returning here let a
    // stopped mission run to completion (measured 2026-09-10).
    if (press.kind === 'cancel-when-named') {
      cancelWhenNamedRef.current.add(press.key)
      setRuns((all) => withRun(all, press.key, markedCancelling))
      return
    }
    const runId = press.runId
    setRuns((all) => withRun(all, runId, markedCancelling))
    void bridge
      .cancelCodexMission({ runId })
      .then((response) => {
        if (response.ok) return
        setRuns((all) =>
          withRun(all, runId, (run) =>
            liveRunIsActive(run) ? { ...run, phase: 'running', error: response.error.message } : run
          )
        )
      })
      .catch(() => {
        setRuns((all) =>
          withRun(all, runId, (run) =>
            liveRunIsActive(run)
              ? { ...run, phase: 'running', error: 'The cancellation request could not be delivered. The mission is still running.' }
              : run
          )
        )
      })
  }

  /** A side question's answer, stopped from its panel (R33): the same cancel a conversation's Stop sends. */
  const stopSideRun = (runId: string): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setRuns((all) => withRun(all, runId, markedCancelling))
    void bridge.cancelCodexMission({ runId }).then((response) => {
      if (response.ok) return
      setRuns((all) => withRun(all, runId, (run) => (liveRunIsActive(run) ? { ...run, phase: 'running', error: response.error.message } : run)))
    }).catch(() => undefined)
  }

  /**
   * The person choosing a route: from the picker, or from a busy model's
   * notice (C9). One path, so the teammate's own pick and the effort move with
   * it whichever way it came.
   */
  const changeRoute = (next: RouteChoice): void => {
    // From here on this route is theirs, and discovery stops
    // moving it.
    routeChosen.current = true
    setRoute(next)
    // For this conversation, or this new chat, only (0.552): the teammate's
    // own model changes in their profile. A new chat with nobody keeps the
    // pick across a restart, as Claude Code's `/model` default does.
    setPickerRoutes((current) => new Map(current).set(pickKey, next))
    if (pickKey === 'new') rememberNewChatRoute(next, mode, effort)
    // Effort belongs to a model, so a level the new model never
    // advertised must not follow it across. But clearing to NOTHING
    // is what made picking a model empty the effort control --
    // Colin, 2026-09-07: "if the user just clicks the model it
    // instantly defaults to no effort, it was cleaner before". So
    // it lands on the new model's default instead.
    // Through `modelFamily`, not a bare id match: a Cursor route
    // resolved to a variant finds no family by id, and the effort
    // control then reads as though the model had none.
    const family = modelFamily(models, next.runtime, next.model)
    setEffort(effortAfterRouteChange(effort, family?.supportedEfforts ?? [], family?.defaultEffort))
  }

  /*
   * ONE PRESS OFF A BUSY FREE MODEL (C9). On 2026-09-26 two of OpenCode's free
   * models were limited for hours while three others answered, and a new
   * person starts on the first of them. The notice said to press Stop and pick
   * another model; this names one -- the next free model after the run's --
   * and the press does what the sentence says: stops the run, puts the chat
   * box on that model and hands the message back, with its files. Enter sends
   * it there. Nothing is sent for the person.
   */
  const busyOffer = ((): { readonly label: string; readonly onPress: () => void } | undefined => {
    const shown = liveRun
    const next = shown?.data === undefined ? undefined : nextFreeModel(shown.data.runtime, shown.data.model, models, freeDown)
    if (shown === undefined || next === undefined) return undefined
    return {
      // By the chip's own name for it: the catalogue's displayName for an
      // OpenCode model is its id ("longcat-2.5-preview-free", the drive's
      // first press), where the chip says "Longcat 2.5 Preview Free".
      label: `Stop and switch to ${modelDisplayName(next.runtime, next.id)}`,
      onPress: () => {
        const split = splitAttachments(shown.prompt)
        cancelMission()
        changeRoute({ runtime: next.runtime, model: next.id })
        setHandBack({ text: split.text, attachments: split.attachments })
      }
    }
  })()

  /*
   * AND WHEN IT GAVE UP (QA-2026-09-29 round 2, R17). The busy note and its
   * button leave when the run ends, and the red card said to pick another
   * model with nothing to press. The same offer, on the ended run: the chat
   * box goes to the next free model with the message back in it. Nothing is
   * sent for the person, as above. Only for the free models, where the next
   * one is known to cost nothing; a paid runtime's limit is the person's call.
   */
  const limitOffer = ((): { readonly label: string; readonly onPress: () => void; readonly why: 'retired' | 'down' | 'limit' } | undefined => {
    const shown = liveRun
    if (shown === undefined || shown.phase !== 'failed' || shown.data === undefined) return undefined
    const failed = [...shown.events].reverse().find((event) => event.type === 'run.failed')
    if (failed === undefined || failed.type !== 'run.failed') return undefined
    // A model OpenCode retired (0.710) is out of the list, so there is no "next after it": the free start instead,
    // one that has answered here first.
    const retired = modelRetired(failed.payload)
    // A free model whose provider is down (0.711) is a limit of its own: the same model fails again.
    const down = !retired && modelUnavailable(failed.payload)
    if (!retired && !down && !failedOnItsLimit(failed.payload)) return undefined
    // Never the model that just failed, which stays in the list until the record of this run reaches it.
    const others = models.filter((model) => model.id !== shown.data?.model)
    const start = retired ? freeStartModel(shown.data.runtime, others, freeAnswered, freeDown) : undefined
    const next = retired ? others.find((model) => model.runtime === shown.data?.runtime && model.id === start) : nextFreeModel(shown.data.runtime, shown.data.model, models, freeDown)
    if (next === undefined) return undefined
    return {
      // What the sentence over the button says: a retired model is not "at its limit" (0.712).
      why: retired ? 'retired' : down ? 'down' : 'limit',
      label: `Switch to ${modelDisplayName(next.runtime, next.id)}`,
      onPress: () => {
        const split = splitAttachments(shown.prompt)
        changeRoute({ runtime: next.runtime, model: next.id })
        setHandBack({ text: split.text, attachments: split.attachments })
      }
    }
  })()

  /*
   * THE PROVIDER WAS BUSY (0.511, missionView.failedOnProviderSide). A run
   * that did real work and then hit "Selected model is at capacity" is
   * carried on, not repeated: Continue sends a follow-up into the same
   * conversation, which picks up the runtime's own session where it stopped.
   * The press is the person's send; nothing goes without it.
   */
  const continueOffer = ((): (() => void) | undefined => {
    const shown = liveRun
    if (shown === undefined || shown.phase !== 'failed' || shown.data === undefined || limitOffer !== undefined) return undefined
    const failed = [...shown.events].reverse().find((event) => event.type === 'run.failed')
    if (failed === undefined || failed.type !== 'run.failed' || !failedOnProviderSide(failed.payload)) return undefined
    return () => void startMission('Continue from where you stopped.')
  })()

  /*
   * WHEN A PLAN RUNS OUT (0.774, shared/carry-on.ts). A paid agent's limit was "the person's call" with nothing to
   * press: pick another model, retype, send. The card names the agent that can carry on -- the one picked in Settings,
   * or the first signed in and not at its own limit -- and one press starts it, briefed with what was done. The host
   * starts the turn (main/carry-on.ts), so the press is the same path as carrying on without one.
   */
  // The agents that can carry work on here: found, signed in, built, and Antigravity only through its command line
  // (main/index.ts) -- the person's own models and anything else discovery lists are not agents to hand a turn to.
  const carryOnAgents: readonly MissionRuntimeId[] = runtimes
    .filter((status) => runtimeIsUsable(status) && integrationOf(status.id) !== 'planned')
    .filter((status) => status.id !== 'antigravity' || status.throughCli === true)
    .map((status) => status.id)
    .filter((id): id is Exclude<typeof id, 'omniroute'> => isMissionRuntime(id))
  const carryOnReady = new Set<MissionRuntimeId>(carryOnAgents.filter((runtime) => !limitedRuntimes.has(runtime)))
  // What Settings offers to carry work on with: every agent that can run a turn here, the chosen one even when it cannot right now.
  const carryOnChoices: readonly { readonly runtime: MissionRuntimeId; readonly label: string }[] = (() => {
    const named = (runtime: MissionRuntimeId): string =>
      planRunsOut?.runtime === runtime && planRunsOut.model !== undefined ? `${runtimeDisplayName(runtime)} · ${modelDisplayName(runtime, planRunsOut.model)}` : runtimeDisplayName(runtime)
    const listed = carryOnAgents.map((runtime) => ({ runtime, label: named(runtime) }))
    const chosen = planRunsOut?.runtime
    return chosen === undefined || listed.some((choice) => choice.runtime === chosen) ? listed : [...listed, { runtime: chosen, label: `${named(chosen)} (not ready)` }]
  })()
  const carryOnShown = ((): { readonly label: string; readonly from: MissionRuntimeId; readonly busy: boolean; readonly refused?: string; readonly onPress: () => void } | undefined => {
    const shown = liveRun
    if (shown === undefined || shown.phase !== 'failed' || shown.data === undefined || limitOffer !== undefined) return undefined
    if (!planRanOut(shown.events)) return undefined
    const from = shown.data.runtime
    const route = carryOnOffer(from, planRunsOut, carryOnReady)
    if (route === undefined) return undefined
    const missionId = shown.data.missionId
    const pressed = carryingOn?.missionId === missionId ? carryingOn : undefined
    const busy = pressed !== undefined && pressed.refused === undefined
    return {
      from,
      label: `Carry on with ${route.model === undefined ? runtimeDisplayName(route.runtime) : modelDisplayName(route.runtime, route.model)}`,
      busy,
      ...(pressed?.refused === undefined ? {} : { refused: pressed.refused }),
      onPress: () => {
        const bridge = window.desktop
        if (!bridge || busy) return
        setCarryingOn({ missionId })
        void bridge
          .carryOn(missionId, route)
          // Started: the new turn's own start clears this. Refused: the reason stays under the button.
          .then((response) => { if (!response.ok) setCarryingOn({ missionId, refused: response.message }) })
          .catch(() => setCarryingOn({ missionId, refused: 'The turn could not be started. The work stays where it stopped.' }))
      }
    }
  })()

  /*
   * A SIGN-IN THAT EXPIRED (QA-2026-09-29 round 2, N11). Codex's `login
   * status` only reads that a sign-in is saved, so Settings kept saying READY
   * after "Your access token could not be refreshed", and the card had nothing
   * to press. The runtime says why it failed; the card offers its sign-in.
   */
  const signInOffer = ((): MissionRuntimeId | undefined => {
    const shown = liveRun
    if (shown === undefined || shown.phase !== 'failed' || shown.data === undefined) return undefined
    const failed = [...shown.events].reverse().find((event) => event.type === 'run.failed')
    if (failed === undefined || failed.type !== 'run.failed' || failed.payload.kind !== 'authentication-failed') return undefined
    return signInCommand(shown.data.runtime) === undefined ? undefined : shown.data.runtime
  })()

  // Returns the save, so the dialog can hold its button until it lands (L22).
  const createTeammate = ({ monthlyLimitUsd, instructions, voice, ...input }: TeammateDraft): Promise<void> => {
    const bridge = window.desktop
    if (!bridge) return Promise.resolve()
    return bridge
      // A new teammate has no limit to lift, nor instructions to clear: a value, or nothing.
      .createTeammate({ ...input, ...(typeof monthlyLimitUsd === 'number' ? { monthlyLimitUsd } : {}), ...(typeof instructions === 'string' ? { instructions } : {}), ...(typeof voice === 'string' ? { voice } : {}) })
      .then((response) => {
        if (!response.ok) {
          setTeammateError(response.error.message)
          return
        }
        setTeammateError(undefined)
        setNewTeammateOpen(false)
        return bridge.listTeammates().then((listed) => {
          if (!listed.ok) return
          setTeammates(listed.data.teammates)
          setMissionOwners(listed.data.missionOwners)
          rememberRosterNames(listed.data)
        })
      })
      .catch(() => setTeammateError('That teammate could not be created. Nobody was added.'))
  }

  /*
   * A WHOLE TEAM FROM A TEMPLATE (0.354), made one teammate at a time through
   * the same create the dialog uses, so each is an ordinary teammate. One that
   * will not make stops the rest and says why; any already made stay, and the
   * roster is read back either way, so Home shows exactly what exists.
   */
  const makeTeamFrom = async (templateId: TeamTemplate['templateId']): Promise<string | undefined> => {
    const bridge = window.desktop
    const template = TEAM_TEMPLATES.find((entry) => entry.templateId === templateId)
    if (bridge === undefined || template === undefined) return 'That team could not be made here.'
    let problem: string | undefined
    for (const mate of template.teammates) {
      const response = await bridge
        .createTeammate({
          name: mate.name,
          hue: mate.hue,
          role: mate.role,
          ...(mate.roleTitle === undefined ? {} : { roleTitle: mate.roleTitle }),
          starters: mate.starters,
          avatar: templateAvatar(templateId, mate)
        })
        .catch(() => undefined)
      if (response === undefined || !response.ok) {
        problem = response === undefined ? `${mate.name} could not be made.` : response.error.message
        break
      }
    }
    const listed = await bridge.listTeammates().catch(() => undefined)
    if (listed?.ok === true) {
      setTeammates(listed.data.teammates)
      setMissionOwners(listed.data.missionOwners)
      rememberRosterNames(listed.data)
    }
    return problem
  }

  const updateTeammate = (
    teammateId: string,
    input: TeammateDraft
  ): Promise<void> => {
    const bridge = window.desktop
    if (!bridge) return Promise.resolve()
    return bridge
      .updateTeammate({ teammateId, ...input })
      .then((response) => {
        if (!response.ok) {
          setTeammateError(response.error.message)
          return
        }
        setTeammateError(undefined)
        setEditingTeammate(undefined)
        return bridge.listTeammates().then((listed) => {
          if (!listed.ok) return
          setTeammates(listed.data.teammates)
          setMissionOwners(listed.data.missionOwners)
          rememberRosterNames(listed.data)
        })
      })
      .catch(() => setTeammateError('That teammate could not be updated. Their details are unchanged.'))
  }

  /**
   * Point one teammate at its own folder, or put it back in the project one.
   *
   * The host opens the dialog and writes the record; what comes back is the
   * teammate, never a path. The open dialog is refreshed from that answer so
   * the row shows the new folder without a Save, which is honest -- the
   * change has already happened by then.
   */
  /** Every connector the host reports, read when the edit dialog opens. */
  const [connectorList, setConnectorList] = useState<readonly PublicConnector[]>()
  useEffect(() => {
    if (editingTeammate === undefined) return
    let live = true
    void window.desktop
      ?.listConnectors()
      .then((response) => {
        if (live && response.ok) setConnectorList(response.data.connectors)
      })
      .catch(() => undefined)
    return () => {
      live = false
    }
  }, [editingTeammate?.teammateId])

  /** Narrow one teammate to some connectors, or widen it back. The host answers with the teammate. */
  const setTeammateConnectors = (teammateId: string, names: readonly string[]): void => {
    const bridge = window.desktop
    if (!bridge) return
    void bridge
      .setTeammateConnectors(teammateId, names)
      .then((response) => {
        if (!response.ok) {
          setFolderNotice(response.error.message)
          return
        }
        setEditingTeammate(response.data.teammate)
        return bridge.listTeammates().then((listed) => {
          if (!listed.ok) return
          setTeammates(listed.data.teammates)
          setMissionOwners(listed.data.missionOwners)
          rememberRosterNames(listed.data)
        })
      })
      .catch(() => setFolderNotice('That could not be changed. The setting is as it was.'))
  }

  const chooseTeammateFolder = (teammateId: string, clear: boolean): void => {
    const bridge = window.desktop
    if (!bridge) return
    setFolderNotice(undefined)
    void bridge
      .chooseTeammateFolder(teammateId, clear)
      .then((response) => {
        if (!response.ok) {
          // Closing the picker is not a failure and is not reported as one.
          if (response.error.code !== 'CANCELLED') setFolderNotice(response.error.message)
          return
        }
        setEditingTeammate(response.data.teammate)
        return bridge.listTeammates().then((listed) => {
          if (!listed.ok) return
          setTeammates(listed.data.teammates)
          setMissionOwners(listed.data.missionOwners)
          rememberRosterNames(listed.data)
        })
      })
      .catch(() => setFolderNotice('That folder could not be chosen. The workspace is unchanged.'))
  }

  const reloadRoutines = async (): Promise<void> => {
    const bridge = window.desktop
    if (!bridge) return
    const listed = await bridge.listRoutines()
    noteStore('routines', listed.ok)
    if (listed.ok) setRoutines(listed.data.routines)
  }

  /**
   * A routine written from nothing (0.530): one empty step, on the picked
   * teammate's own route (or the dialog asks who runs it), learned from no
   * conversation. Sol's 0.528 pass had no other way in than replaying one.
   */
  const openNewRoutine = (): void => {
    const teammate = pickedTeammate ?? (teammates.length === 1 ? teammates[0] : undefined)
    // A teammate with no conversation yet has no route of its own: it would
    // run on what the box shows, so the dialog offers that, and "What a run
    // may do" has a mode to start from (Sol, 0.532).
    const route = teammate?.route ?? (teammate === undefined ? undefined : { ...composerRoute, mode })
    setRoutineDialog({
      teammateId: teammate?.teammateId,
      name: '',
      steps: [''],
      learnedFrom: [],
      truncated: false,
      busy: false,
      ...(route === undefined ? {} : { route })
    })
  }

  /** Open the dialog on a draft taken from a finished conversation. */
  const openSaveRoutine = (missionId: string): void => {
    const mission = historyByIdRef.current.get(missionId)
    if (mission === undefined) return
    const draft = routineDraftFor(missionId)
    if (draft === undefined) return
    /*
     * A CONVERSATION NOBODY OWNS CAN STILL BE SAVED, and the dialog asks who
     * will run it.
     *
     * This returned early when the conversation had no owner, so the last
     * hole in the solo path was this one: memory and the folder brief reached
     * an ownerless run in 0.191.0 and Save as routine still did not. The
     * steps are the PERSON's words either way -- that is the whole of what a
     * routine is -- and the only thing genuinely missing is whose route
     * replays them, which is a question with an answer on screen.
     */
    /*
     * The conversation's owner, walked back -- not this turn's.
     *
     * A follow-up typed after a conversation was assigned records no owner of
     * its own, and reading the turn alone is exactly the defect Grok found on
     * pass 14: the menu told the person who had just typed the words that
     * nothing here was typed by them. Asking "who runs it" about a
     * conversation that plainly has an owner would be the same mistake in a
     * new place.
     */
    const teammateId = conversationOwnerOf(missionId)
    const teammate = teammates.find((entry) => entry.teammateId === teammateId)
    setRoutineDialog({
      teammateId,
      name: draft.name,
      steps: draft.steps,
      learnedFrom: draft.learnedFrom,
      truncated: draft.truncated,
      // The route the routine will replay on: the teammate's own, else the
      // one this conversation actually ran on. Never a guess.
      //
      // The effort comes from the composer, because missions do not record
      // theirs -- so there is no level to recover from the conversation being
      // taught, and the one on screen is the level the next run would use.
      //
      // Only where it would actually be SENT as one. The first version of this
      // stored it whenever the composer had a level, on the reading that a
      // model reporting no levels is exactly a runtime that refuses an effort.
      // That reading is wrong: Cursor reports levels AND refuses the argument,
      // because its levels live inside the model id. So a routine taught on
      // Cursor stored `{model: "composer-2.5-fast", effort: "fast"}`, and
      // replaying it put both in front of a builder that throws on the second
      // -- every routine on Cursor failed to start, silently, from 0.43.0
      // until routine-smoke caught it.
      route: (() => {
        /*
         * The mode the conversation ACTUALLY RAN IN, not `ask`.
         *
         * Colin, 2026-09-21: a routine saved from an ownerless conversation
         * could never run. The conversation ran on Cursor in `auto`; this
         * stored `ask`, and Cursor cannot be held read-only on Windows, so
         * every attempt was refused at dispatch. `ask` was a safe-looking
         * guess that is not safe at all -- it is a mode the person did not
         * choose, on a route that may not support it.
         *
         * A mission records its mode (v15), so there is nothing to guess.
         * Older missions have none; `ask` remains the answer only when the
         * record itself cannot say.
         */
        const base = teammate?.route ?? {
          runtime: mission.runtime,
          model: mission.model ?? 'account-default',
          mode: mission.mode ?? ('ask' as const)
        }
        const carried =
          effort === undefined || effortIsInModelId(models, base.runtime, base.model)
            ? undefined
            : effort
        return { ...base, ...(carried === undefined ? {} : { effort: carried }) }
      })(),
      busy: false
    })
  }

  const saveRoutine = (input: {
    readonly inputs?: readonly RoutineInput[]
    readonly name: string
    readonly steps: readonly string[]
    readonly schedule: RoutineSchedule | undefined
    /** Who takes each step (0.435); absent when the dialog offered no choice. */
    readonly handOffs?: readonly RoutineHandOff[]
    /** Who runs it, when the conversation had no owner to inherit. */
    readonly teammateId?: string
    /** Set only when the person changed what a run may do (0.530). */
    readonly readsOnly?: boolean
    /** Where it works (0.533): in the folder, or in a copy the person keeps. */
    readonly inCopy?: boolean
    /** Keep going until the check passes (0.534); null takes it away. */
    readonly untilCheck?: { readonly tries: number } | null
  }): void => {
    const bridge = window.desktop
    const dialog = routineDialog
    if (!bridge || dialog === undefined) return
    // The dialog asks for one when the conversation had none; this is the
     // second half of that rule, so a routine can never be stored ownerless.
    const owner = input.teammateId ?? dialog.teammateId
    /*
     * The route follows whoever the person just NAMED.
     *
     * `dialog.route` is decided when the dialog opens, and for a
     * conversation nobody owns there is no teammate to read it from -- so it
     * fell back to the conversation's runtime with a guessed mode, and
     * answering "who runs this?" never revisited it. The routine was then
     * stored on a mode its own teammate had never run in. Same rule as the
     * dialog's: the teammate's own route, else what the dialog worked out.
     */
    const named = teammates.find((entry) => entry.teammateId === owner)?.route
    const inherited = named ?? dialog.route ?? { runtime: 'codex' as const, model: 'account-default', mode: 'ask' as const }
    // "Only read" is Ask; "Change files" from a read-only mode is Edit; otherwise the mode stays as it was.
    const chosenMode: MissionMode | undefined = input.readsOnly === undefined ? dialog.routineId === undefined ? dialog.forceMode : undefined : input.readsOnly ? 'ask' : 'accept-edits'
    const route = chosenMode === undefined ? inherited : { ...inherited, mode: chosenMode }
    if (dialog.routineId === undefined && owner === undefined) {
      setRoutineDialog({ ...dialog, busy: false, error: 'Choose which teammate runs this routine.' })
      return
    }
    setRoutineDialog({ ...dialog, busy: true, error: undefined })
    const request =
      dialog.routineId === undefined
        ? bridge.createRoutine({
            name: input.name,
            teammateId: owner ?? '',
            route,
            steps: input.steps,
            learnedFrom: dialog.learnedFrom,
            ...(input.inputs === undefined ? {} : { inputs: input.inputs }),
            ...(input.schedule === undefined ? {} : { schedule: input.schedule }),
            ...(input.handOffs === undefined ? {} : { handOffs: input.handOffs }),
            ...(input.inCopy === true ? { inCopy: true } : {}),
            ...(input.untilCheck === undefined || input.untilCheck === null ? {} : { untilCheck: input.untilCheck })
          })
        : // null clears a schedule the routine had; the store leaves an absent one alone.
          bridge.updateRoutine({ routineId: dialog.routineId, name: input.name, steps: input.steps, schedule: input.schedule ?? null, ...(input.inputs === undefined ? {} : { inputs: input.inputs }), ...(input.handOffs === undefined ? {} : { handOffs: input.handOffs }), ...(chosenMode === undefined ? {} : { mode: chosenMode }), ...(input.inCopy === undefined ? {} : { inCopy: input.inCopy }), ...(input.untilCheck === undefined ? {} : { untilCheck: input.untilCheck }) })
    void request
      .then(async (response) => {
        if (!response.ok) {
          setRoutineDialog({ ...dialog, busy: false, error: response.error.message })
          return
        }
        setRoutineDialog(undefined)
        await reloadRoutines()
      })
      .catch(() => setRoutineDialog({ ...dialog, busy: false, error: 'That routine could not be saved. The version on disk is unchanged.' }))
  }

  /**
   * Open a saved routine for editing. Shared by the roster card and the
   * Automations screen: two copies of this had already started to drift in
   * the same file.
   */
  const editRoutine = (routine: PublicRoutine): void => {
    setRoutineDialog({
      teammateId: routine.teammateId,
      routineId: routine.routineId,
      name: routine.name,
      steps: routine.steps,
      ...(routine.inputs === undefined ? {} : { inputs: routine.inputs }),
      learnedFrom: routine.learnedFrom,
      truncated: false,
      ...(routine.schedule === undefined ? {} : { schedule: routine.schedule }),
      ...(routine.handOffs === undefined ? {} : { handOffs: routine.handOffs }),
      savedMode: routine.route.mode,
      running: routine.execution?.status === 'running' || routine.execution?.status === 'dispatching',
      ...(routine.inCopy === true ? { inCopy: true } : {}),
      ...(routine.untilCheck === undefined ? {} : { goalTries: routine.untilCheck.tries }),
      busy: false
    })
  }

  /** A copy routine's waiting changes: kept, discarded, or the copy opened (0.533). */
  const settleRoutine = (routineId: string, decision: 'keep' | 'discard' | 'open'): void => {
    void window.desktop
      ?.settleRoutine({ routineId, decision })
      .then(async (answer) => {
        if (answer.message.length > 0) setRoutineNotice(answer.message)
        if (decision !== 'open') await reloadRoutines()
      })
      .catch(() => setRoutineNotice('That could not be done. Nothing was changed.'))
  }

  /** M30: a routine run to show once it is in the window. */
  const followRunRef = useRef<string | undefined>(undefined)
  useEffect(() => {
    const want = followRunRef.current
    if (want === undefined) return
    const key = runs.has(want) ? want : [...runs.entries()].find(([, run]) => run.data?.runId === want)?.[0]
    if (key === undefined) return
    followRunRef.current = undefined
    setShownKey(key)
    const owner = runs.get(key)?.teammateId
    if (owner !== undefined) setSelectedTeammateId(owner)
  }, [runs])
  const [routineNotice, setRoutineNotice] = useState<string>()
  const [routineRunFor, setRoutineRunFor] = useState<PublicRoutine>()
  const [routineImport, setRoutineImport] = useState<RoutineImportPreview & { readonly fromTemplate?: boolean }>()
  // The starter routines (0.615): read once, the first time Routines opens; they ship with the app.
  const [routineTemplates, setRoutineTemplates] = useState<readonly RoutineTemplateInfo[]>([])
  const [templatePicker, setTemplatePicker] = useState(false)
  useEffect(() => {
    if (screen !== 'automations' || routineTemplates.length > 0) return
    void window.desktop?.listRoutineTemplates().then((answer) => { if (answer.ok) setRoutineTemplates(answer.data.templates) }).catch(() => undefined)
  }, [screen, routineTemplates.length])
  const openRoutineTemplate = (id: string): void => {
    void window.desktop?.previewRoutineTemplate(id).then((answer) => {
      if (!answer.ok) { setRoutineNotice(answer.error.message); return }
      setTemplatePicker(false)
      const preview = answer.data.preview
      if (preview === undefined) return
      /*
       * A CHAIN opens in the editor, not in the preview that gives every step
       * to one teammate: its steps name roles, a teammate is proposed for each
       * from this roster, and the person sees and changes who takes what
       * before anything is saved. The ones that change files open set to
       * change them, in a copy that waits for Keep or Discard.
       */
      if (preview.handOffChecks.some((checks) => checks)) {
        setRoutineDialog({ ...chainDraftFrom(preview, teammates, { ...composerRoute, mode }), learnedFrom: [], truncated: false, busy: false })
        return
      }
      setRoutineImport({ ...preview, fromTemplate: true })
    }).catch(() => setRoutineNotice('That template could not be opened. Try it again from Routines.'))
  }
  const [routineExport, setRoutineExport] = useState<{ routineId: string; flagged: readonly RoutineFlaggedPath[] }>()
  const exportRoutine = async (routineId: string, paths?: 'input' | 'keep'): Promise<void> => {
    if (!window.desktop) throw new Error('Desktop connection is unavailable.')
    const answer = await window.desktop.exportRoutine({ routineId, ...(paths === undefined ? {} : { paths }) })
    if (!answer.ok) {
      if (answer.error.code === 'ABSOLUTE_PATHS') { setRoutineExport({ routineId, flagged: answer.error.flagged }); return }
      throw new Error(answer.error.message)
    }
    if (!answer.data.cancelled) setRoutineNotice(`Exported to ${answer.data.path ?? 'a routine file'}.`)
  }
  const previewRoutineImport = (): void => {
    void window.desktop?.previewRoutineImport().then((answer) => {
      if (!answer.ok) { setRoutineNotice(answer.error.message); return }
      setRoutineImport(answer.data.preview)
    }).catch(() => setRoutineNotice('That routine file could not be opened. Choose a readable .locust-routine.json file.'))
  }
  const startRoutine = async (routineId: string, values?: RoutineValues): Promise<void> => {
    if (!window.desktop) throw new Error('Desktop connection is unavailable.')
    const response = await window.desktop.runRoutine(routineId, values)
    if (!response.ok) throw new Error(response.error.message)
    setRoutineNotice(undefined)
    followRunRef.current = response.data.runId
    setScreen('workroom')
    await reloadRoutines()
  }
  const runRoutine = (routineId: string): void => {
    const routine = routines.find((entry) => entry.routineId === routineId)
    if (routine?.inputs?.length) { setRoutineRunFor(routine); return }
    void startRoutine(routineId).catch((error: unknown) => setRoutineNotice(error instanceof Error ? error.message : 'The routine start could not be confirmed. Check its conversation before starting it again.'))
  }

  const recoverRoutine: import('./components/RoutineRecovery.js').RecoverRoutine = async (request) => {
    if (!window.desktop) return { ok: false, error: { message: 'Desktop connection is unavailable.' } }
    const response = await window.desktop.recoverRoutine(request)
    await reloadRoutines()
    return response
  }

  const pauseRoutine = (routineId: string, paused: boolean): void => {
    void window.desktop?.pauseRoutine(routineId, paused)
      .then((answer) => { if (!answer.ok) setRoutineNotice(answer.error.message) })
      .then(() => reloadRoutines())
      .catch(() => setRoutineNotice('That routine could not be changed. Its schedule is as it was.'))
  }

  const removeRoutine = (routineId: string): void => {
    const bridge = window.desktop
    if (!bridge) return
    void bridge
      .removeRoutine(routineId)
      .then(() => reloadRoutines())
      .catch(() => undefined)
  }

  const removeTeammate = (teammateId: string): void => {
    const bridge = window.desktop
    if (!bridge) return
    /*
     * A TEAMMATE REMOVED MID-RUN STOPS (QA-2026-09-29 round 2, R22). Their
     * run carried on with nobody's name on it, writing to the folder for a
     * teammate that no longer existed. Stopped first, as their own Stop would.
     */
    for (const [key, run] of runsRef.current) {
      if (!liveRunIsActive(run) || ownerOf(run) !== teammateId) continue
      const runId = run.data?.runId ?? key
      setRuns((all) => withRun(all, key, markedCancelling))
      void bridge.cancelCodexMission({ runId }).catch(() => undefined)
    }
    void bridge
      .removeTeammate(teammateId)
      .then(() => bridge.listTeammates())
      .then(async (listed) => {
        if (!listed.ok) return
        // Not left picked: see `addressedTeammate`.
        setSelectedTeammateId((current) => (current === teammateId ? undefined : current))
        setTeammates(listed.data.teammates)
        setMissionOwners(listed.data.missionOwners)
        rememberRosterNames(listed.data)
        // Their routines went with them; the host drops those, so re-read.
        await reloadRoutines()
      })
      .catch(() => undefined)
  }

  /*
   * The boot screen is its own WINDOW now, not part of this one.
   *
   * It used to be folded into the pane here, with a phase machine, a
   * discovery subscription and a clock all living in the app shell. All of
   * that moved to `SplashApp`, which is shown before this window exists and
   * closes when the runtimes have answered. Leaving a second copy here is
   * how the same ceremony came to play twice and finish in neither.
   */

  const running = liveRunIsActive(liveRun)
  /*
   * Whether the folder is a git repository (2026-10-16). The Changes panel is a branch's changes against its base,
   * so a folder that is not a repository has none: on a computer without Git its button only ever opened "This
   * folder is not a git repository" (the clean-machine drive). Read per folder and after each run, since a teammate
   * may run git init; while git has not said, the button stays.
   */
  const [folderRepository, setFolderRepository] = useState<{ readonly path: string; readonly repository: boolean | undefined }>()
  useEffect(() => {
    if (workspacePath === undefined || running) return
    let current = true
    void window.desktop
      ?.folderIsRepository()
      .then((repository) => {
        if (current) setFolderRepository({ path: workspacePath, repository })
      })
      .catch(() => undefined)
    return () => {
      current = false
    }
  }, [workspacePath, running])
  const changesAvailable = folderRepository?.path !== workspacePath || folderRepository?.repository !== false
  /**
   * What THIS mission cost, for the line that is about this mission.
   *
   * It used to be the conversation's total, and Grok measured what that
   * does (2026-09-14, finding 2): a mission cancelled before anything
   * started -- its own ledger holding a create, a cancel and no usage at
   * all -- wore "1.8k in · 189 out", which were the previous turn's exact
   * counts. A run that failed on a provider error wore the turn before
   * it. Every other part of that line is a fact about this mission: its
   * id, its model, its phase, its sandbox. A stranger reads the number
   * beside them as this run's, and it was not.
   *
   * Silence when this run reported nothing, which is the honest answer for
   * a run that never reached a model -- never a zero, which reads as free.
   */
  /**
   * The conversation's total, for the conversation-scoped surface.
   *
   * Design ruling, 2026-09-14: this does not belong in the mission strip,
   * which is present-tense and mission-scoped. It lives beside the route
   * chip -- inside the context ring's hover where a ring exists, and stated
   * outright in the ring's slot where one does not, which is every runtime
   * except Claude Code.
   */
  const shownConversationCost =
    liveRun === undefined ? undefined : conversationCostLine(liveRun.earlierTurns ?? [], liveRun.events)
  const shownCostTail =
    liveRun === undefined
      ? ''
      : headerCostTail({ events: liveRun.events, running })
  /**
   * How full the model's context is, from the newest turn that reported it.
   * Not the conversation's summed tokens: the window holds one prompt, so
   * adding turns together would say a five-turn chat is five times as full.
   */
  const shownContext =
    liveRun === undefined ? undefined : latestContext(liveRun.earlierTurns ?? [], liveRun.events)
  const runningCount = [...runs.values()].filter(liveRunIsActive).length

  /**
   * Pick the folder the teammates work in. A chosen folder reopens the app
   * there, which stops anything running -- so with runs live it says so and
   * does nothing, rather than taking the person's work down with the switch.
   */
  const chooseWorkspace = (): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setWorkspaceNotice(undefined)
    // In place (0.458): runs keep going in their own folders, nothing restarts.
    void bridge
      .chooseWorkspace()
      .then((response) => {
        if (response.ok) {
          // A folder chosen is where the next conversation starts (0.458).
          setShownKey(undefined)
          setFolderQuestion(undefined)
          refreshForFolder()
        } else if (response.error.code !== 'CANCELLED') setWorkspaceNotice(response.error.message)
      })
      .catch(() => setWorkspaceNotice('The folder could not be chosen. The workspace is unchanged.'))
  }
  /** Start here, on the question over the box (0.790): the folder is kept as said yes to, then the box sends. */
  const confirmFolder = async (): Promise<boolean> => {
    const kept = await window.desktop?.confirmWorkspace().catch(() => false) ?? false
    if (kept) setFolderQuestion(undefined)
    return kept
  }
  const historyById = useMemo(
    () => new Map(history.map((mission) => [mission.missionId, mission] as const)),
    [history]
  )

  // Re-read history whenever ANY run settles, so a finished mission stays in
  // the sidebar after the next one starts instead of vanishing until restart.
  const settledSignature = [...runs.entries()]
    .filter(([, run]) => isTerminal(run.phase))
    .map(([key]) => key)
    .sort()
    .join('|')
  useEffect(() => {
    if (settledSignature.length === 0) return
    refreshHistory()
    // A tree that was "In use" is free once its run settles. Settings read
    // the list when it opened and kept saying In use until the NEXT run
    // (seen driving the app, 2026-09-05); re-read once the list has ever
    // been asked for, so a person sitting on Settings sees Remove appear.
    if (worktrees !== undefined) refreshWorktrees()
  }, [settledSignature])

  // A queued message goes as the next turn the moment its run COMPLETES, and
  // only then. A run that failed, was stopped or was interrupted leaves it
  // waiting with the reason on screen: sending the next instruction into a
  // conversation whose last turn did not happen would build on work that
  // never ran, which is the same rule a routine's steps follow.
  //
  // Only the conversation ON SCREEN sends, because a turn is started into the
  // conversation on screen: a message queued elsewhere waits in its own
  // conversation and goes when that is opened again. Each conversation is
  // judged by its own FRONT -- the row that goes next there, which every row
  // behind it is waiting on too -- and never by another's: one queue for
  // every conversation made a line for Gem wait on Pip's run and then ride
  // along with Pip's (outside beta recheck of 0.299, P1).
  const front = queuedIn(queued, shownKey)[0]
  // M32: judged by the run it waits behind, when that is another conversation's.
  const queuedRun = front === undefined ? undefined : runs.get(front.waitFor ?? front.key)
  const verdict =
    front === undefined
      ? undefined
      : queuedVerdict({
          running: queuedRun !== undefined && liveRunIsActive(queuedRun),
          phase: queuedRun?.phase,
          restored: front.restored,
          held: front.held,
          onScreen: true
        })
  /** Ticks when a queued message's retry time comes round, so the effect below looks again. */
  const [queueClock, setQueueClock] = useState(0)
  useEffect(() => {
    if (front === undefined || shownKey === undefined || verdict?.kind !== 'send') return undefined
    // Refused as busy a moment ago: not before its retry time.
    const wait = (front.retryAt ?? 0) - Date.now()
    if (wait > 0) {
      const timer = window.setTimeout(() => setQueueClock((tick) => tick + 1), wait)
      return () => window.clearTimeout(timer)
    }
    // Folded at the moment of sending rather than as it is typed, so a person
    // can still edit or drop any row right up until it goes.
    // Cleared BEFORE sending: this effect runs again on the state the send
    // produces, and a queue still holding the message would send it twice.
    // Only what actually went is dropped -- anything the fold refused to
    // merge stays queued and takes its own turn, and every other
    // conversation's rows stay where they are.
    void sendQueued(shownKey, (going) => startMission(withAttachments(going.text, going.attachments ?? []), undefined, { requeue: going }))
    return undefined
  }, [queued, front, shownKey, verdict?.kind, queueClock])

  /*
   * STOP AND SEND NOW (0.485), Claude Code's ctrl+enter: the queued message
   * goes at once instead of when the teammate finishes. The stop lands first;
   * then, if the run ended stopped (or failed), the queue's next message goes
   * into that conversation. A run that happened to finish normally in the
   * meantime is left to the queue above, which sends it the usual way -- so it
   * is never sent twice.
   */
  const [sendAfterStop, setSendAfterStop] = useState<string>()
  useEffect(() => {
    if (sendAfterStop === undefined) return
    const run = runs.get(sendAfterStop)
    if (run !== undefined && liveRunIsActive(run)) return
    setSendAfterStop(undefined)
    if (run === undefined || run.phase === 'completed') return
    void sendQueued(sendAfterStop, (going) => startMission(withAttachments(going.text, going.attachments ?? []), undefined, { requeue: going }))
  }, [sendAfterStop, runs, queued])

  /** The addressed teammate's live run, if they have one: they cannot be given a second. */
  /*
   * The run a message waits behind: the live one ON SCREEN, and no other
   * (0.551). It was the addressed teammate's live run anywhere, so a new
   * conversation with a teammate busy in another waited for that one to end.
   * Colin: "we should 100% be able to have multiple convos with the same
   * teammate going". Each conversation takes its own turns.
   */
  const busyRun = shownKey !== undefined && liveRunIsActive(runs.get(shownKey)) ? runs.get(shownKey) : undefined

  /*
   * THE CONVERSATION THE BOX QUEUES INTO, AND SHOWS THE QUEUE OF: the live
   * run on screen -- that is the conversation being replied into -- else the
   * addressed teammate's busy run, else the conversation on screen, for what
   * is still waiting there after its run ended. Keyed on the addressed
   * teammate's busy run first, queueing did nothing at all on a fresh
   * profile, where every mission belongs to nobody (steering smoke,
   * 2026-09-05); the teammate's run is the fallback for when the thread on
   * screen is someone else's.
   *
   * One key for both, so what the box shows as NEXT is exactly what typing
   * adds to, and Edit, Discard and Send act on that and nothing else.
   */
  const liveOnScreen = shownKey !== undefined && liveRunIsActive(runs.get(shownKey)) ? shownKey : undefined
  const busyKey = busyRun === undefined ? undefined : [...runs.entries()].find(([, run]) => run === busyRun)?.[0]
  /*
   * M32: a message typed into one of the addressed teammate's OWN
   * conversations while they are busy in another stays in the one it was
   * typed in, waiting for the other run: shown and sent there. Only a thread
   * that is not theirs -- someone else's, or none -- queues into their busy
   * run, as before.
   */
  const shownRun = shownKey === undefined ? undefined : runs.get(shownKey)
  const shownIsTheirs = shownRun !== undefined && pickedTeammate !== undefined && ownerOf(shownRun) === pickedTeammate.teammateId
  const { queueKey, waitForKey } = queueHome({ liveOnScreen, busyKey, shownKey, shownIsTheirs })
  const waitingHere = queuedIn(queued, queueKey)
  const waitingRunKey = waitingHere[0]?.waitFor ?? queueKey
  const waitingRun = waitingRunKey === undefined ? undefined : runs.get(waitingRunKey)
  const waitingVerdict =
    waitingHere[0] === undefined
      ? undefined
      : queuedVerdict({
          running: waitingRun !== undefined && liveRunIsActive(waitingRun),
          phase: waitingRun?.phase,
          restored: waitingHere[0].restored,
          held: waitingHere[0].held,
          onScreen: queueKey === shownKey
        })
  const queuedNote = waitingVerdict?.kind === 'held' ? waitingVerdict.note : undefined

  /**
   * Show a run's thread. A run the shell already knows about is shown as it
   * is, live or not; anything else is reopened from the ledger. A
   * continuation opens stitched -- the root's prompt, the prior run's events,
   * the divider rebuilt from the checkpoint, then this run -- because that is
   * what the durable record says happened.
   */
  /**
   * Point the composer at the conversation now on screen.
   *
   * The route is remembered per teammate, but the composer's own route starts
   * at Codex and nothing moved it when a conversation was reopened -- so a
   * restored Claude thread sat above a composer saying `Codex CLI /
   * account-default`, and the next message would have gone somewhere the
   * person never chose (QA pass, 2026-09-05). What the run actually ran on is
   * in its own record, so it is read from there rather than guessed.
   *
   * Never for a run that is still going: the composer already follows a live
   * run's own route, and moving the controls under a person mid-mission is
   * its own surprise.
   */
  const followRouteOf = (run: LiveRunState | undefined): void => {
    if (run?.data === undefined || liveRunIsActive(run)) return
    setRoute({ runtime: run.data.runtime, model: run.data.model ?? 'account-default', ...(routeAccount(run.data) === undefined ? {} : { account: routeAccount(run.data) }) })
    /*
     * The MODE too, from the record, and the effort from the teammate.
     *
     * Opening a conversation used to seed the runtime and model and leave
     * the mode wherever the last screen left it. Colin, 2026-09-16: "ill
     * have stocks convo on auto so it can access the mcp tools, the next
     * time it will be accept edits and fail the mcp call because i dont
     * notice." The conversation's last turn recorded its mode; that is what
     * the next turn should start on. Effort is not recorded per mission, so
     * it follows the teammate's remembered route, which now keeps it.
     */
    const ownerId = run.teammateId ?? (run.data.missionId === undefined ? undefined : missionOwnersRef.current[run.data.missionId])
    const own = ownerId === undefined ? undefined : teammates.find((teammate) => teammate.teammateId === ownerId)?.route
    // A run of THIS session has no record read back yet; its teammate's route
    // is remembered at every start, mode included, so it names the mode that
    // run used. Without it a routine step opened fresh -- Sable, Codex, Ask --
    // showed whatever the last screen left: "Edit" (0.435 hand-off drive).
    const recorded = run.restoredMission?.mode ?? (own !== undefined && own.runtime === run.data.runtime ? own.mode : undefined)
    if (recorded !== undefined && modeRunsOn(recorded, run.data.runtime, build?.platform)) setMode(recorded)
    if (own !== undefined && own.runtime === run.data.runtime) setEffort(own.effort)
  }

  /*
   * FOLDERS LIKE CLAUDE CODE (0.458, docs/PLAN-2026-09-29-FOLDERS-LIKE-CLAUDE-CODE.md).
   * Everything the window holds about ITS folder, read again after a switch:
   * the name and path, this folder's check command, LOCUST.md and the
   * runtimes' setup, the / commands, memory, routines, the worktrees, and the
   * history's idea of which folder is current. Nothing restarts.
   */
  const [folders, setFolders] = useState<readonly PublicFolder[]>([])
  const refreshFolders = (): void => {
    void window.desktop?.listFolders().then((listed) => setFolders(folderLabels(listed.folders))).catch(() => undefined)
  }
  useEffect(() => { refreshFolders() }, [history.length])
  const refreshForFolder = (): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    void bridge.getAppInfo().then((info) => {
      setWorkspaceName(info.workspaceName)
      setWorkspacePath(info.workspacePath.length === 0 ? undefined : info.workspacePath)
      setWorkspaceMade(info.workspaceMade === true)
      setWorkspaceCaution(info.workspaceCaution)
      // Another folder: the question, if any, was about the one before.
      setFolderQuestion(undefined)
    }).catch(() => undefined)
    void bridge.readWorkspaceSettings().then((settings) => setCheckCommand(settings.checkCommand ?? '')).catch(() => undefined)
    refreshRuntimeSetup()
    rereadRuntimeCommands()
    refreshMemories()
    void bridge.listRoutines().then((response) => {
      if (response.ok) setRoutines(response.data.routines)
    }).catch(() => undefined)
    if (worktrees !== undefined) refreshWorktrees()
    refreshFolders()
    void bridge.getMissionHistory(heldDigests(historyRef.current)).then((response) => {
      seedLimitsFrom(response)
      applyHistory(response)
      if (response.ok) setWorkspaceId(response.data.currentWorkspaceId)
    }).catch(() => undefined)
  }
  /** Put the window in a folder already worked in, by its id. */
  const switchToFolder = (id: string): void => {
    void window.desktop?.switchFolder(id).then((answer) => {
      if (!answer.ok) {
        setWorkspaceNotice(answer.message)
        return
      }
      setWorkspaceNotice(undefined)
      setWorkspaceId(answer.folder.id)
      refreshForFolder()
    }).catch(() => undefined)
  }

  /*
   * H3: AN OLDER CONVERSATION IS READ WHEN IT IS OPENED. History sends the
   * newest missions whole and the rest as rows, and this is where the rows
   * of a conversation -- every turn of it -- are fetched, then the thread is
   * built again from the whole records. It used to open on the person's
   * words and no replies, with "Events: 41 recorded" beside them.
   *
   * Its own function since QA-2026-09-29 round 2, R1: the conversation
   * adopted at launch was already among the runs, so opening it took the
   * early return above and never came here -- a 30-turn conversation
   * reopened after a restart with turns 1 to 10 unanswered.
   */
  const readTurnsOf = (mission: PublicRecoveredMission): void => {
    const missing = missingTranscripts([mission.missionId, ...conversationTurns(mission, historyById).map((turn) => turn.missionId)], historyById)
    const bridge = window.desktop
    if (missing.length === 0 || bridge?.readMission === undefined) return
    void Promise.all(missing.map((id) => bridge.readMission(id).catch(() => undefined))).then((answers) => {
      const read = new Map(
        answers.flatMap((answer) => (answer !== undefined && answer.ok ? [[answer.data.mission.missionId, answer.data.mission] as const] : []))
      )
      if (read.size === 0) return
      setHistory((current) => current.map((entry) => read.get(entry.missionId) ?? entry))
      const whole = new Map(historyById)
      for (const [id, entry] of read) whole.set(id, entry)
      const now = whole.get(mission.missionId)
      if (now === undefined) return
      // Only a thread that is still the reopened record is rebuilt: a turn
      // started meanwhile is live, and its own events are the truth.
      setRuns((current) => {
        const shown = current.get(mission.runId)
        return shown === undefined || !isTerminal(shown.phase) ? current : withNewRun(current, mission.runId, reopenedRun(now, whole))
      })
    })
  }

  const openMission = (missionId: string): void => {
    // Opening a conversation leaves any comparison on screen (0.441).
    setComparingId(undefined)
    // And a side chat, which was about the conversation it was opened on (0.461).
    setSideChat((current) => (current === undefined || current.of === missionId ? current : undefined))
    // A conversation from another folder takes the window there, as Claude
    // Code's sessions do (0.458): the folder chip, @ files and pages are its.
    const itsFolder = historyById.get(missionId)?.workspaceId
    // Not into a Locust install, where early builds ran: that one is only read.
    const readOnly = folders.find((folder) => folder.id === itsFolder)?.installFolder === true
    if (itsFolder !== undefined && workspaceId !== undefined && itsFolder !== workspaceId && !readOnly) switchToFolder(itsFolder)
    // Opening a conversation SHOWS it. Every caller but one used to have to
    // remember `setScreen('workroom')` first, and the sidebar did not -- so
    // from the Missions screen a click lit the row and left you looking at
    // the list, with no thread and no composer (QA pass, 2026-09-05). The
    // screen change belongs to the action, not to each caller.
    setScreen('workroom')
    // Whoever the mission belongs to is who the composer now addresses. Done
    // here rather than in each caller, because the sidebar, the Missions list
    // and a teammate's message in an exchange all arrive through this one
    // function and had disagreed about it. The run's own answer comes first,
    // which is what the header uses, so the two cannot disagree while a run
    // is waiting to be recorded.
    const runHere =
      runs.get(missionId) ?? [...runs.values()].find((run) => run.data?.missionId === missionId)
    setSelectedTeammateId((current) =>
      ownerToSelect(missionId, missionOwnersRef.current, current, runHere?.teammateId)
    )
    // A run that is still starting is listed under its pending key.
    if (runs.has(missionId)) {
      setShownKey(missionId)
      followRouteOf(runs.get(missionId))
      return
    }
    const known = [...runs.entries()].find(([, run]) => run.data?.missionId === missionId)
    if (known !== undefined) {
      setShownKey(known[0])
      followRouteOf(known[1])
      // A finished one may be a record adopted without its older turns (R1).
      const held = historyById.get(missionId)
      if (held !== undefined && isTerminal(known[1].phase)) {
        /*
         * ITS VERSIONS, AS THEY ARE NOW (0.512). A run drawn earlier in this
         * session kept the versions it had then: Sol's pass on 0.509 edited
         * a message, carried the new branch on, pressed "Show the version
         * before" -- and the old branch, run before the edit existed, had no
         * "Show the edited version" until Locust was restarted and rebuilt it
         * from history. History knows both branches; the turns are told again.
         */
        const told = new Map(conversationTurns(held, historyById).map((turn) => [turn.missionId, turn.versions] as const))
        setRuns((current) => {
          const shown = current.get(known[0])
          if (shown === undefined || !isTerminal(shown.phase)) return current
          const { versions: _was, ...rest } = shown
          const last = told.get(missionId)
          return withNewRun(current, known[0], {
            ...rest,
            ...(shown.earlierTurns === undefined
              ? {}
              : {
                  earlierTurns: shown.earlierTurns.map((turn) => {
                    const { versions: _old, ...plain } = turn
                    const now = told.get(turn.missionId)
                    return now === undefined ? plain : { ...plain, versions: now }
                  })
                }),
            ...(last === undefined ? {} : { versions: last })
          })
        })
        readTurnsOf(held)
      }
      return
    }
    const mission = historyById.get(missionId)
    if (mission === undefined) return
    const reopened = reopenedRun(mission, historyById)
    setSelectedTeammateId((current) => ownerToSelect(missionId, missionOwnersRef.current, current))
    setRuns((current) => withNewRun(current, mission.runId, reopened))
    setShownKey(mission.runId)
    followRouteOf(reopened)
    readTurnsOf(mission)
  }

  /*
   * WHAT HAPPENED IN THE TERMINAL COMES BACK (0.391).
   *
   * The `</>` button opens a conversation in its runtime's own terminal. What
   * the person does there is asked for when the conversation is shown, and
   * again whenever the window comes back into focus on it: the host records
   * it as turns of the conversation, and the window opens the newest once
   * history holds it, so the thread shows it where it happened. Only a
   * settled conversation, on a runtime whose sessions the host can read.
   */
  const catchUpId =
    liveRun?.data !== undefined && !liveRunIsActive(liveRun) && (liveRun.data.runtime === 'claude' || liveRun.data.runtime === 'codex')
      ? liveRun.data.missionId
      : undefined
  const caughtUp = useRef<{ readonly from: string; readonly to: string } | undefined>(undefined)
  useEffect(() => {
    const bridge = window.desktop
    if (catchUpId === undefined || bridge?.catchUpTerminal === undefined) return
    let gone = false
    const ask = (): void => {
      void bridge
        .catchUpTerminal(catchUpId)
        .then((result) => {
          if (gone || result.imported === 0 || result.latestMissionId === undefined) return
          // Whose they are, before anything is drawn: a message is a REPLY
          // only when the turn on screen is its teammate's.
          const owners = result.owners
          if (owners !== undefined) setMissionOwners((current) => ({ ...current, ...owners }))
          caughtUp.current = { from: catchUpId, to: result.latestMissionId }
          refreshHistory()
        })
        .catch(() => undefined)
    }
    ask()
    window.addEventListener('focus', ask)
    return () => {
      gone = true
      window.removeEventListener('focus', ask)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [catchUpId])
  useEffect(() => {
    const pending = caughtUp.current
    if (pending === undefined || !historyById.has(pending.to)) return
    caughtUp.current = undefined
    // Only while the conversation it came back into is still the one shown.
    if (liveRun?.data?.missionId === pending.from) openMission(pending.to)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [historyById])

  /**
   * Delete the shown mission's record for good. The host refuses while it is
   * live, and that refusal is shown rather than swallowed. On success the
   * mission leaves every list it was in and the thread empties, because
   * showing a thread whose record is gone would be showing a ghost.
   */
  /**
   * The conversation on screen, when it can be saved as a routine.
   *
   * Absent rather than disabled -- see `savableMissionId`. The draft check is
   * the same one the right-click menu makes, so the two entrances can never
   * disagree about whether there is anything to save.
   */
  const saveAsRoutineId = savableMissionId({
    missionId: liveRun?.data?.missionId,
    phase: liveRun?.phase,
    running,
    hasDraft:
      liveRun?.data?.missionId !== undefined && routineDraftFor(liveRun.data.missionId) !== undefined
  })
  const [deleteError, setDeleteError] = useState<string>()

  /**
   * Hand this finished mission to another teammate to be challenged.
   *
   * Astra's proposal, part 2, and it waited for part 1a on purpose: until a
   * turn said what it RAN, a reviewer had nothing to read but the diff, and a
   * second model re-reading a diff is a second opinion about code rather than
   * a check on whether the work was done.
   *
   * Everything the reviewer is given is already on this screen. The brief
   * exists because a teammate cannot see another teammate's conversation, so
   * the facts have to travel -- and because a reviewer told the work passed
   * and then asked to check it has been handed the answer. See reviewBrief.ts.
   */
  const reviewMaterialFor = (run: LiveRunState): ReviewMaterial | undefined => {
    const events = run.events
    if (events.length === 0) return undefined
    const activity = buildThread(events, { running: false }).find((item) => item.type === 'activity')
    const details = activity?.type === 'activity' ? activity.details : []
    const changed = [
      ...new Set(
        details
          .filter((detail) => detail.kind === 'edit' && detail.failed !== true)
          // A Codex change of several files names them one per line; each is
          // its own entry, not one joined path (a B4 lead).
          .flatMap((detail) => detail.name.split('\n').map((name) => name.trim()).filter((name) => name.length > 0))
          .map((name) => relativePath(name, workspacePath))
      )
    ]
    // A run the host started carries an assembled briefing as its prompt, so
    // only a person-typed turn can speak for itself.
    const thisTurnAsk = run.startedBy === undefined ? run.prompt : undefined
    const rootAsk = run.earlierTurns?.[0]?.prompt
    const commands = details
      .filter((detail) => detail.kind === 'shell')
      .map((detail) => ({
        name: shellCommandText(detail.name).split('\n')[0]?.trim() ?? detail.name,
        ...(detail.exitCode === undefined ? {} : { exitCode: detail.exitCode })
      }))
    return {
      /*
       * THIS turn's request, and the conversation's opening ask beside it.
       *
       * It was the ROOT request alone, on the reasoning that a continuation's
       * own prompt is the host's briefing -- true for a run the HOST started
       * (a relay hand-off, a routine step, another review), and false for a
       * follow-up a person typed, whose prompt is their words.
       *
       * Astra caught what that cost, 2026-09-14, in a real four-turn
       * conversation: turn one asked "explain git ... don't change any
       * files", turn four asked for two files to be created, and the review
       * of turn four was handed turn ONE as WHAT WAS ASKED FOR. The reviewer
       * was being asked to judge file-creating work against an instruction
       * not to create files -- a mismatch that would make correct work read
       * as a violation.
       */
      request: thisTurnAsk ?? rootAsk ?? run.prompt,
      ...(rootAsk !== undefined && thisTurnAsk !== undefined && rootAsk.trim() !== thisTurnAsk.trim()
        ? { openedWith: rootAsk }
        : {}),
      // The reply, which for research or a question IS the work. Its absence
      // is what made a reviewer say "the work is missing entirely" about a
      // page of analysis on 2026-09-13.
      said: turnText(events),
      changed,
      commands: commands.map((command) => ({ name: command.name, exitCode: command.exitCode })),
      /*
       * What the CONVERSATION built, not just this turn.
       *
       * `changed` above is this run's events alone, which is the whole work
       * only when the conversation is one turn long. A teammate that created
       * a file on turn two and adjusted it on turn four had its turn four
       * reviewed against a single path, and a reviewer asked whether the
       * request and the evidence agree says -- correctly, on what it was
       * shown -- that they do not.
       *
       * Read from the same `earlierTurns` the thread draws, so this costs no
       * new source of truth, and handed over as its own labelled list rather
       * than merged, so the reviewer can still tell which turn is its job.
       */
      changedEarlier: [
        ...new Set(
          (run.earlierTurns ?? []).flatMap((turn) => {
            const earlier = buildThread(turn.events, { running: false }).find((item) => item.type === 'activity')
            const theirs = earlier?.type === 'activity' ? earlier.details : []
            return theirs
              .filter((detail) => detail.kind === 'edit' && detail.failed !== true)
              .map((detail) => relativePath(detail.name, workspacePath))
          })
        )
      ],
      ranOn: ranOnLine({
        platform: window.desktop?.platform ?? '',
        ...(folderName(workspacePath) === undefined ? {} : { folder: folderName(workspacePath)! })
      }),
      author: teammates.find((entry) => entry.teammateId === ownerOf(run))?.name ?? 'A teammate'
    }
  }

  /** Every teammate who could review this one -- anyone but its author. */
  const reviewersFor = (run: LiveRunState | undefined): readonly PublicTeammate[] =>
    run === undefined ? [] : teammates.filter((entry) => entry.teammateId !== ownerOf(run))

  /**
   * A tidy pass (A1.2): one of the team reads the folder's memories and
   * suggests merges, retirements and rewrites -- an ordinary run of theirs,
   * in their own thread, as a review is. Its suggestions come back as
   * proposals on the Memory screen; nothing changes until the person keeps
   * one.
   */
  const tidyMemory = (teammateId: string): void => {
    const teammate = teammates.find((one) => one.teammateId === teammateId)
    if (teammate === undefined) return
    selectTeammate(teammateId)
    // As THEM, whole (H9): not the closure's idea of who is on screen.
    void startMission(TIDY_PROMPT, undefined, { as: startAs(teammate, { route, mode, effort }, pickerRoutes) })
  }
  /*
   * SCOUT ON HOME (0.756, ScoutCards.tsx). Send starts the drafted turn as that teammate's own -- a new conversation
   * of theirs, as a tidy pass is -- and Change it first puts the draft in the box addressed to them; either way the
   * card has done its job. Open goes to the conversation or the page on GitHub. Turning Scout on adds it to the team
   * on the steadiest free model listed here (freeStartModel, best first, passing over one that is down).
   */
  const [scoutBusy, setScoutBusy] = useState(false)
  /** Look now, pressed on Home (0.768): its line says Looking… until the look is in. */
  const [scoutLooking, setScoutLooking] = useState(false)
  const lookWithScout = (): void => {
    if (scoutLooking) return
    setScoutLooking(true)
    void window.desktop?.scout?.look().catch(() => undefined).finally(() => { setScoutLooking(false) })
  }
  const [scoutMessage, setScoutMessage] = useState<string>()
  const [scoutNotNow, setScoutNotNow] = useState(() => {
    try { return window.localStorage.getItem(SCOUT_NOT_NOW) === '1' } catch { return false }
  })
  const scoutModel = freeStartModel('opencode', models, [], freeDown)
  /** "Space Bunny Free through OpenCode": the model's own name as the team card says it, its agent, and what it costs. */
  const scoutModelWords = (on: { readonly runtime: MissionRuntimeId; readonly model: string }): string => {
    const name = homeRouteOf(on, resolvedModels, models).model
    const free = /(^|[-/])free$/i.test(on.model) && !/\bfree\b/i.test(name) ? ', free' : ''
    return on.runtime === 'opencode' ? `${name} through OpenCode${free}` : `${name}${free}`
  }
  const turnOnScout = async (): Promise<void> => {
    const bridge = window.desktop
    if (bridge?.scout === undefined || scoutModel === undefined || scoutBusy) return
    setScoutBusy(true)
    setScoutMessage(undefined)
    try {
      const answer = await bridge.scout.turnOn({ runtime: 'opencode', model: scoutModel })
      if ('refused' in answer) { setScoutMessage(answer.refused); return }
      const listed = await bridge.listTeammates().catch(() => undefined)
      if (listed?.ok === true) {
        setTeammates(listed.data.teammates)
        setMissionOwners(listed.data.missionOwners)
        rememberRosterNames(listed.data)
      }
    } catch {
      setScoutMessage('Scout could not be turned on. Nothing was changed; try again.')
    } finally {
      setScoutBusy(false)
    }
  }
  const scoutNotNowPressed = (): void => {
    setScoutNotNow(true)
    setScoutInviteOpen(false)
    try { window.localStorage.setItem(SCOUT_NOT_NOW, '1') } catch { /* asked again next time */ }
  }
  /** Meet Scout, open over Home (0.808): opened from Scout's offered card on the team. */
  const [scoutInviteOpen, setScoutInviteOpen] = useState(false)
  const openScoutCard = (open: ScoutOpen): void => {
    if (open.kind === 'conversation') openMission(open.missionId)
    else void window.desktop?.openLink(open.url).catch(() => undefined)
  }
  const sendScoutDraft = (draft: ScoutDraft, cardId: string): void => {
    const teammate = teammates.find((one) => one.teammateId === draft.teammateId)
    if (teammate === undefined) return
    selectTeammate(teammate.teammateId)
    // As THEM, whole (H9), on their own route and mode: a new conversation of theirs.
    void startMission(draft.message, undefined, { as: startAs(teammate, { route, mode, effort }, pickerRoutes) })
    void window.desktop?.scout?.dismiss(cardId).catch(() => undefined)
  }
  const draftScoutIntoBox = (draft: ScoutDraft, cardId: string): void => {
    if (!teammates.some((one) => one.teammateId === draft.teammateId)) return
    selectTeammate(draft.teammateId)
    setComposerFill((current) => ({ text: draft.message, seq: (current?.seq ?? 0) + 1 }))
    void window.desktop?.scout?.dismiss(cardId).catch(() => undefined)
  }
  const openTidyMenu = (anchor: HTMLElement): void => {
    if (rowMenu?.anchor === anchor) {
      setRowMenu(undefined)
      return
    }
    const box = anchor.getBoundingClientRect()
    setRowMenuArmed(undefined)
    setRowMenu({
      x: box.left,
      y: box.bottom + 4,
      title: 'Tidy up with',
      anchor,
      items:
        teammates.length === 0
          ? [{ label: 'Add a teammate first', disabledReason: 'A tidy pass is a teammate reading the memories.' }]
          : teammates.map((teammate) => ({ label: `Ask ${teammate.name}`, onSelect: () => tidyMemory(teammate.teammateId) }))
    })
  }
  const askForReview = (run: LiveRunState, reviewer: PublicTeammate): void => {
    const material = reviewMaterialFor(run)
    if (material === undefined) return
    selectTeammate(reviewer.teammateId)
    /*
     * As the REVIEWER, whole (H9). This used to wait a tick "so the run is
     * started as theirs" -- and then call the startMission of the render
     * before the selection, which read the AUTHOR's teammate, route and
     * mode: the author reviewed their own work and the reviewer never ran
     * (code review H9, and the probe's own capture). A review is an
     * ordinary mission of the reviewer's, on their own route, as a new
     * conversation of theirs.
     */
    void startMission(reviewBrief(material), undefined, { as: asReviewer(startAs(reviewer, { route, mode, effort }, pickerRoutes), antigravityCli) })
  }
  /**
   * A3.1: the same review from two teammates on DIFFERENT coding agents, at
   * once -- each an ordinary review of theirs, in their own conversation,
   * with the reviewer contract (edit nothing; a verdict line first). Two
   * readings by two models is the thing a one-model tool cannot offer.
   */
  const reviewPairFor = (run: LiveRunState) =>
    reviewPairOf(
      reviewersFor(run).map((one) => ({ ...one, runtime: startAs(one, { route, mode, effort }, pickerRoutes).route.runtime })),
      run.data?.runtime ?? run.runtime
    )
  const askTwoForReview = (run: LiveRunState, pair: readonly [PublicTeammate, PublicTeammate]): void => {
    const material = reviewMaterialFor(run)
    if (material === undefined) return
    const brief = reviewBrief(material)
    selectTeammate(pair[0].teammateId)
    void (async () => {
      for (const reviewer of pair) {
        await startMission(brief, undefined, { as: asReviewer(startAs(reviewer, { route, mode, effort }, pickerRoutes), antigravityCli) })
      }
    })()
  }
  /**
   * What a row action just did, when it worked.
   *
   * A refusal has had somewhere to appear since 2026-09-05. Success had
   * nowhere, and that is the other half of the same complaint: assigning a
   * conversation moves a row in a list you may not be looking at, so a
   * successful assign and a failed one look identical from the composer --
   * "i right clicked it and hit assign to wren, nothing happened" (Colin,
   * 2026-09-08, on a build where assign demonstrably worked).
   */
  const [rowNotice, setRowNotice] = useState<string>()
  // The team as a picture of itself (0.398): the Share dialog, and what adding one did.
  const [sharingTeam, setSharingTeam] = useState(false)
  const [teamNotice, setTeamNotice] = useState<string>()
  const addTeamFromCard = (fromHome = false): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setTeamNotice(undefined)
    void bridge
      .addTeamFromCard()
      .then(async (answer) => {
        if (!answer.ok) {
          setTeamNotice(answer.message)
          if (fromHome) setScreen('teammates')
          return
        }
        if (answer.added.length === 0 && answer.skipped.length === 0) return
        const roster = await bridge.listTeammates()
        if (roster.ok) setTeammates(roster.data.teammates)
        const added = answer.added.length === 0 ? 'No one was added.' : `Added ${answer.added.join(', ')} from the team card.`
        // From Home, the Team screen is where the team, and this sentence, are.
        if (fromHome) setScreen('teammates')
        const skipped = answer.skipped.length === 0 ? '' : ` ${answer.skipped.join(', ')} could not be added: the card's details for them did not read.`
        // What came with them (0.802): instructions are someone else's words, so where to read them is said.
        const instructed = answer.instructed.length === 0 ? '' : ` ${answer.instructed.join(', ')} came with instructions: read them in Edit teammate first.`
        const routines = answer.routines.length === 0
          ? ''
          : answer.routines.length === 1
            ? ` ${answer.routines[0]!} came too, paused: it runs only when you start it, read-only.`
            : ` ${String(answer.routines.length)} routines came too, paused: ${answer.routines.join(', ')}. They run only when you start them, read-only.`
        const left = answer.routinesLeft.length === 0 ? '' : ` Not brought: ${answer.routinesLeft.join(', ')}.`
        setTeamNotice(`${added}${skipped}${instructed}${routines}${left}`)
      })
      .catch(() => setTeamNotice('That image could not be read as a Locust team card.'))
  }
  // It confirms a thing that already happened, so it goes away on its own. A
  // standing green line would become furniture, and furniture is not read.
  useEffect(() => {
    if (rowNotice === undefined) return
    const clear = window.setTimeout(() => setRowNotice(undefined), 4_000)
    return () => window.clearTimeout(clear)
  }, [rowNotice])
  /*
   * THE KEYS THAT START TALK FROM ANY APP (0.755, shared/talk-shortcut.ts): main brings the window up, and the Talk
   * button on screen takes the press -- or the chat box's, once the window is back on it. Talk not ready says why,
   * where the other one-press answers are said, instead of a call that would fail.
   */
  const [talkNotice, setTalkNotice] = useState<string>()
  useEffect(() => {
    const shortcut = window.desktop?.talkShortcut
    if (shortcut === undefined) return
    const take = (press: TalkShortcutPress): void => {
      if (press.why !== undefined) { setScreen('workroom'); setTalkNotice(`Talk isn’t ready: ${press.why}`); return }
      if (!pressTalk()) setScreen('workroom')
    }
    const stop = shortcut.onPressed(take)
    // A press made while the window was opening (Locust started in the tray).
    void shortcut.pending().then((press) => { if (press !== undefined) take(press) }).catch(() => undefined)
    return stop
  }, [])
  useEffect(() => {
    if (talkNotice === undefined) return
    const clear = window.setTimeout(() => setTalkNotice(undefined), 8_000)
    return () => window.clearTimeout(clear)
  }, [talkNotice])
  /*
   * A LINK THAT OPENS LOCUST WITH A MESSAGE READY (0.770, shared/locust-link.ts): Home, as the logo opens it, with
   * the message in the box and a word over it that it came from a link. Never sent: Send is the person's.
   */
  useEffect(() => {
    const links = window.desktop?.locustLinks
    if (links === undefined) return
    let live = true
    const take = (): void => {
      void links.take().then((link) => {
        if (!live || link === undefined) return
        setSelectedTeammateId(undefined)
        setShownKey(undefined)
        setComparingId(undefined)
        setRoomNotice(undefined)
        setScreen('workroom')
        setComposerFill((current) => ({ text: link.prompt, seq: (current?.seq ?? 0) + 1, from: 'link' }))
      }).catch(() => undefined)
    }
    // One that started Locust, or arrived as it opened, is already waiting.
    take()
    const stop = links.onWaiting(take)
    return () => {
      live = false
      stop()
    }
  }, [])
  /*
   * Why a conversation did not open in a terminal (0.387), said where the
   * delete refusal is said, and gone on its own: it answers one press.
   */
  const [terminalError, setTerminalError] = useState<string>()
  useEffect(() => {
    if (terminalError === undefined) return
    const clear = window.setTimeout(() => setTerminalError(undefined), 8_000)
    return () => window.clearTimeout(clear)
  }, [terminalError])
  const openConversationInTerminal = (missionId: string, offer: TerminalOffer): void => {
    setTerminalError(undefined)
    const bridge = window.desktop
    if (bridge === undefined) return
    void bridge
      .openInTerminal(missionId)
      .then((answer) => {
        if (answer.ok) setRowNotice(offer.opened(answer.where))
        else setTerminalError(answer.message)
      })
      .catch(() => setTerminalError('The terminal could not be opened. The conversation is here, as it was.'))
  }
  /*
   * The trash, from the window's side.
   *
   * A restore has to put the conversation back where a person looks for it,
   * and that is two reads: the history the sidebar is built from, and the
   * roster that carries who owns it and what it was renamed to. Neither is
   * derivable from the other.
   */
  const listTrash = async (): Promise<TrashListResponse> => {
    const bridge = window.desktop
    if (!bridge) return { ok: false, error: { code: 'STORAGE_UNAVAILABLE', message: 'The trash could not be read. Nothing in it has been deleted for good.' } }
    return bridge.listTrashedMissions()
  }

  const restoreMission = async (missionId: string): Promise<TrashMutationResponse> => {
    const bridge = window.desktop
    if (!bridge) return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'That conversation could not be put back. It is still in the trash.' } }
    const response = await bridge.restoreMission(missionId)
    if (response.ok) {
      refreshHistory()
      void bridge
        .listTeammates()
        .then((roster) => {
          if (!roster.ok) return
          setMissionOwners(roster.data.missionOwners)
          rememberRosterNames(roster.data)
        })
        .catch(() => undefined)
    }
    return response
  }

  const emptyTrash = async (): Promise<TrashMutationResponse> => {
    const bridge = window.desktop
    if (!bridge) return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The trash was not emptied. Everything in it is still there.' } }
    return bridge.emptyTrash()
  }

  const deleteMissionById = (missionId: string): void => {
    const bridge = window.desktop
    if (!bridge) return
    void bridge
      .deleteMission(missionId)
      .then((response) => {
        if (!response.ok) {
          setDeleteError(response.error.message)
          return
        }
        setDeleteError(undefined)
        setRuns((current) => {
          const next = new Map(current)
          for (const [key, run] of current) if (run.data?.missionId === missionId) next.delete(key)
          return next
        })
        setHistory((current) => current.filter((mission) => mission.missionId !== missionId))
        setMissionOwners((current) => {
          const { [missionId]: _gone, ...rest } = current
          return rest
        })
        setShownKey((current) => {
          const shown = current === undefined ? undefined : runsRef.current.get(current)
          // Only the thread that was deleted closes. Deleting a row from the
          // sidebar must not take the workroom with it.
          return shown?.data?.missionId === missionId ? undefined : current
        })
        void refreshStorage()
      })
      .catch(() => {
        setDeleteError('That could not be deleted. Its record is still here.')
      })
  }

  /** Address a teammate, and look at what they are doing (or their idle state). */
  const selectTeammate = (teammateId: string): void => {
    setSelectedTeammateId(teammateId)
    setScreen('workroom')
    // Picking a teammate picks their route: the composer shows the runtime,
    // model and mode they last ran on, so a person is not re-choosing a
    // model every time they switch who they are talking to.
    const own = teammates.find((teammate) => teammate.teammateId === teammateId)?.route
    if (own !== undefined) {
      setRoute({ runtime: own.runtime, model: own.model, ...(own.account === undefined ? {} : { account: own.account }) })
      setMode(own.mode)
      /*
       * Including the effort, which this used to drop.
       *
       * `TeammateRoute` carries one, and its own docstring says why -- "how
       * hard the model was asked to think is part of how it ran". Picking the
       * teammate restored their runtime, model and mode and then silently
       * reset the effort to the model's default, so a teammate saved at `low`
       * ran at `medium` every time they were selected. Seen while driving
       * approve-each on 2026-09-08: the seed said `low`, the composer said
       * `medium`, and the run cost more than it was asked to.
       *
       * This is the same shape as the bug `routine-store.ts` guards against by
       * naming every field it copies -- an object rebuilt field by field
       * quietly loses anything nobody remembered. `undefined` is a real value
       * here: a teammate on a runtime with no levels must CLEAR the effort,
       * not inherit the last one.
       */
      setEffort(own.effort)
    }
    const theirs = [...runs.entries()].filter(([, run]) => ownerOf(run) === teammateId)
    const startedAt = (run: LiveRunState): number =>
      Date.parse(run.restoredMission?.lastUpdatedAt ?? run.events[0]?.occurredAt ?? '') || 0
    const newest = [...theirs].sort(([, left], [, right]) => startedAt(right) - startedAt(left))[0]
    const live = theirs.find(([, run]) => liveRunIsActive(run)) ?? newest
    setShownKey(live?.[0])
  }

  /**
   * A blank page with that teammate on it.
   *
   * "New conversation with Jimothy" called `selectTeammate`, which ends by
   * showing that teammate's newest conversation -- and since you reach the
   * control by clicking their avatar, they were already selected and their
   * newest conversation was already open. So the button did nothing at all
   * (Colin, 2026-09-14: "this new convo button does nothing").
   *
   * Selecting them takes their route, model, mode and effort, which is what
   * a new conversation with them should start on; then the conversation
   * itself is cleared, which is the part that was missing.
   */
  const newConversationWith = (teammateId: string): void => {
    selectTeammate(teammateId)
    setShownKey(undefined)
  }

  /**
   * A face's click: their hub, or them.
   *
   * The hub is the conversation their replies to other teammates land in,
   * recorded by the host as its newest turn. It is opened through the
   * sidebar's own row for it when one is listed -- the same path a click on
   * the row takes -- so the thread shows the whole conversation and not
   * just the turn the host happened to record. A teammate with no hub yet
   * has nothing to open, so the click addresses them, as it always did.
   */
  const openHub = (teammateId: string): void => {
    const hub = teammates.find((teammate) => teammate.teammateId === teammateId)?.hubMissionId
    if (hub === undefined) {
      selectTeammate(teammateId)
      return
    }
    const row = sidebarMissionsRef.current.find((entry) => (entry.memberIds ?? [entry.missionId]).includes(hub))
    openMission(row?.missionId ?? hub)
  }

  const sidebarMissionsRef = useRef<readonly SidebarMission[]>([])
  /** Which folder each live run's row was first drawn in (0.458). */
  const runFolders = useRef(new Map<string, string>())
  // The right-click menus are built outside render, so they read the roster
  // and the routines through refs the same way they read the rows.
  const teammatesRef = useRef<readonly PublicTeammate[]>([])
  const routinesRef = useRef<readonly PublicRoutine[]>([])
  /*
   * The history's rows, built when the history changes rather than on every streamed word: `runs` changes with
   * each delta, and the whole list was rebuilt with it -- at 2,000 missions with a check inside the loop that
   * walked every row already built (review 2026-10-15, P1).
   */
  const historyRows = useMemo<readonly SidebarMission[]>(() => {
    const rows: SidebarMission[] = []
    for (const mission of history) {
      if (mission.startedBy?.kind === 'side' || hiddenRunIds.has(mission.missionId)) continue
      // Every folder's conversations, each under its own project (0.458, like
      // Claude Code). This used to drop every other folder's, so a folder
      // switch emptied the sidebar and read as the team being reset (Colin,
      // 2026-09-29).
      const root = rootMission(mission, historyById)
      const routineId = routineOf(root.startedBy)
      rows.push({
        missionId: mission.missionId,
        folderId: mission.workspaceId,
        // A continuation's own prompt is the briefing; name it by the words
        // the person typed at the start of the chain.
        title: missionTitle(relayedTitle(root) ?? shownPrompt(root)),
        rootId: root.missionId,
        // What this turn said, for search: a follow-up's words are not in the title (0.414).
        words: splitAttachments(typedPrompt(mission, historyById)).text,
        ...(mission.continuesFrom === undefined ? {} : { parentId: mission.continuesFrom.missionId }),
        ...(routineId === undefined ? {} : { routineId }),
        phase: mission.phase,
        runtime: mission.runtime,
        model: mission.model,
        integrityIssueCount: mission.integrityIssueCount,
        lastAt: mission.lastUpdatedAt
      })
    }
    return rows
  }, [history, historyById, hiddenRunIds])
  const sidebarMissions = useMemo<readonly SidebarMission[]>(() => {
    const rows: SidebarMission[] = []
    const listedLive = new Set<string>()
    for (const [key, run] of runs.entries()) {
      // A send the host REFUSED never became a mission. It was assigned no
      // id and it has already settled, so listing it here files a permanent
      // row under a key like `pending:3` -- a thing that looks like a mission,
      // answers "Copy mission id" with a lie, and can never be reopened.
      // Codex's QA pass on 0.14 caught this as a prompt that "appeared
      // submitted" but "was never recorded as a mission". The refusal still
      // shows in the thread, which is where a person is looking when it
      // happens; what it must not do is accumulate.
      if (!listedAsMission({ missionId: run.data?.missionId, active: liveRunIsActive(run) })) continue
      // A question on the side is drawn in its panel, never as a conversation (0.461); a judge in its comparison (0.520).
      if (run.startedBy?.kind === 'side' || isHiddenRun(run.startedBy, run.data?.missionId)) continue
      // A run that is still starting has no missionId yet; it is listed under
      // its pending key so the teammate reads as working from the first
      // moment, not from the first receipt.
      const missionId = run.data?.missionId ?? key
      if (listedLive.has(missionId)) continue
      listedLive.add(missionId)
      // A live reply already holds the turns before it, so its chain is
      // known without waiting for the ledger to be re-read.
      const earlier = run.earlierTurns ?? []
      // A handoff's continuation knows the mission it continues from its
      // start data, before the ledger is re-read. Without it the pair sat
      // in the sidebar as two rows with one title (seen driving the app,
      // 2026-09-05); the root is resolved through history when it is there.
      const handoffFrom = (run.data as { continuesFrom?: { missionId: string } } | undefined)?.continuesFrom?.missionId
      const handoffRoot = handoffFrom === undefined
        ? undefined
        : historyById.get(handoffFrom) === undefined ? handoffFrom : rootMission(historyById.get(handoffFrom)!, historyById).missionId
      /*
       * THE RECORD OUTRANKS WHAT THE RUN REMEMBERS ABOUT ITS CHAIN.
       *
       * A live run takes its idea of the turns before it when it is opened,
       * and never revisits it. Grok, passes 9 and 10: seed a two-turn
       * conversation, delete the first turn, OPEN the surviving one, put the
       * first turn back. The record now holds both turns again; the open run
       * still remembers having no parent, so the sidebar draws two rows for
       * one conversation -- and a person who then tidies the "duplicate"
       * loses half of it for good. The drive that skipped the open step
       * never saw this. So when the ledger knows this mission, the chain
       * comes from the ledger; what the run remembers is only for a turn
       * the ledger has not been re-read for yet.
       */
      const recorded = historyById.get(missionId)
      const rootId = recorded === undefined
        ? earlier[0]?.missionId ?? handoffRoot
        : rootMission(recorded, historyById).missionId
      const parentId = recorded === undefined
        ? earlier.at(-1)?.missionId ?? handoffFrom
        : recorded.continuesFrom?.missionId
      // A routine's own step says so; a person's reply inside a routine's
      // conversation is known by the conversation's first turn.
      const routineId = routineOf(run.startedBy) ?? routineOf(rootId === undefined ? undefined : historyById.get(rootId)?.startedBy)
      /*
       * Its folder (0.458): the record's when there is one; otherwise the
       * window's folder when this row was first drawn, which is where it was
       * started -- kept, so a switch mid-run does not move it.
       */
      const rootFolder = rootId === undefined ? undefined : historyById.get(rootId)?.workspaceId
      const folderId = recorded?.workspaceId ?? rootFolder ?? runFolders.current.get(missionId) ?? workspaceId
      if (folderId !== undefined && !runFolders.current.has(missionId)) runFolders.current.set(missionId, folderId)
      rows.push({
        missionId,
        ...(folderId === undefined ? {} : { folderId }),
        ...(rootId === undefined ? {} : { rootId }),
        ...(parentId === undefined ? {} : { parentId }),
        ...(routineId === undefined ? {} : { routineId }),
        ...(run.teammateId === undefined ? {} : { ownerId: run.teammateId }),
        /*
         * When this run started, or when the mission it restored last moved.
         *
         * `startedAtIso` is stamped when WE start a run, so a run that was
         * restored from the ledger rather than started has none -- and the
         * newest conversation is precisely the one the app restores on
         * launch. In the nested sidebar that cost nothing, because rows were
         * grouped by teammate and the age was not drawn. In a list ordered
         * most-recent-first it put the most recent conversation LAST, with
         * no age beside it, which is the one row the ordering exists to put
         * at the top. Seen on the first drive of the flat list, 2026-09-15.
         */
        // And once it has moved, when it last did (liveRunLastAt): a finished run is dated by its end, as the ledger dates it.
        ...(liveRunLastAt(run) === undefined ? {} : { lastAt: liveRunLastAt(run)! }),
        // A run's thread already shows the root's words for a continuation.
        // A relayed run's prompt is the host's briefing to a runtime, never a
        // sentence to name a conversation with.
        title: missionTitle(relayedTitle({ ...run, peerMessages: run.peerMessages ?? [] }) ?? shownPrompt(run)),
        phase: liveRunIsActive(run) ? 'running' : isTerminal(run.phase) ? (run.phase as 'completed' | 'failed' | 'cancelled') : 'interrupted',
        ...(run.data?.runtime === undefined ? {} : { runtime: run.data.runtime }),
        // Its model too, so a row with no teammate keeps its logo once opened (0.550, Colin on 0.549).
        ...((run.data?.model ?? recorded?.model) === undefined ? {} : { model: (run.data?.model ?? recorded?.model)! }),
        integrityIssueCount: run.restoredMission?.integrityIssueCount ?? 0
      })
    }
    // A live row stands for its mission; the history's own row for it is not listed again.
    const live = new Set(rows.map((row) => row.missionId))
    for (const row of historyRows) if (!live.has(row.missionId)) rows.push(row)
    // One row per conversation. The ledger still holds one mission per run;
    // this is only how the exchange is listed.
    /*
     * A name someone typed wins over the one derived from their first
     * sentence -- applied AFTER the collapse and keyed by `rootId`, because
     * the thing being named is the conversation and not the turn. Naming a
     * reply and having only that reply change its name would be a rename
     * that did not do what it said.
     *
     * Applied here rather than in the sidebar so every surface agrees: the
     * header, the search that filters on `title`, and the row all read the
     * same string.
     */
    // A comparison is one row until a column is kept (0.441, compareRows.ts).
    const named = foldComparisons(collapseConversations(rows), compares).map((row) => {
      // Asked for under every id this conversation has worn, not only its
      // root -- a name typed while the row was keyed by a live turn was
      // stored against that turn and is otherwise never found again.
      const chosen = heldFor(row, missionTitles)
      return chosen === undefined ? row : { ...row, title: chosen }
    })
    /*
     * A conversation a teammate started for another sits under the one it
     * came from (0.463, nested-conversations.ts): its first turn's received
     * message names the conversation that sent it.
     */
    const sentFrom = (row: SidebarMission): string | undefined => {
      const root = historyById.get(row.rootId ?? row.missionId)
      if (root?.startedBy?.kind !== 'relay') return undefined
      return root.peerMessages.find((message) => message.direction === 'received')?.from.missionId
    }
    const parents = nestedUnder(named, sentFrom)
    return named.map((row) => (parents.has(row.missionId) ? { ...row, nestedUnder: parents.get(row.missionId)! } : row))
  }, [historyRows, historyById, runs, workspaceId, missionTitles, compares, hiddenRunIds])
  // The conversation the box writes in, so an unsent message stays with it (0.551).
  const shownConversation = liveRun?.data?.missionId ?? shownKey
  const shownRow = shownConversation === undefined ? undefined : sidebarMissions.find((entry) => (entry.memberIds ?? [entry.missionId]).includes(shownConversation))
  const composerDraftKey = shownConversation === undefined ? 'new' : shownRow?.rootId ?? shownRow?.missionId ?? shownConversation
  const referenceFolderId = shownConversation === undefined ? workspaceId : historyById.get(shownConversation)?.workspaceId ?? runFolders.current.get(shownConversation) ?? workspaceId
  const referenceCwd = pickedTeammate?.worktree === true
    ? worktrees?.list.find((tree) => tree.teammateId === pickedTeammate.teammateId)?.path
    : pickedTeammate?.folder ?? folders.find((folder) => folder.id === referenceFolderId)?.path ?? workspacePath
  draftKeyRef.current = composerDraftKey
  useEffect(() => {
    const bridge = window.desktop?.ownCommands
    if (bridge === undefined) return undefined
    let gone = false
    const context = {
      ...(liveRun?.data?.missionId === undefined ? {} : { missionId: liveRun.data.missionId }),
      ...(pickedTeammate === undefined ? {} : { teammateId: pickedTeammate.teammateId })
    }
    void bridge.list(context).then((answer) => { if (!gone && answer.ok) setOwnCommandMenu(answer.commands) }).catch(() => undefined)
    return () => { gone = true }
  }, [workspacePath, liveRun?.data?.missionId, pickedTeammate?.teammateId])
  useEffect(() => window.desktop?.ownCommands?.onDone((result) => {
    setCommandEntries((current) => {
      if (!current.some((entry) => entry.id === result.id)) {
        commandResults.current.set(result.id, result)
        return current
      }
      const next = current.map((entry) => entry.id === result.id ? {
        ...entry,
        output: result.output,
        exitCode: result.exitCode,
        stopped: result.stopped,
        running: false,
        ...(result.omitted === undefined ? {} : { omitted: result.omitted })
      } : entry)
      commandEntriesRef.current = next
      return next
    })
  }), [])
  const commandContext = (): { missionId?: string; teammateId?: string } => ({
    ...(liveRunRef.current?.data?.missionId === undefined ? {} : { missionId: liveRunRef.current.data.missionId }),
    ...(pickedTeammate === undefined ? {} : { teammateId: pickedTeammate.teammateId })
  })
  const applyOwnCommand = (answer: { readonly text: string; readonly mode?: MissionMode; readonly teammate?: string }): void => {
    if (answer.mode !== undefined && policyModeAllowed(build?.organizationPolicy, answer.mode)) {
      if (answer.mode === 'auto' && autoMode !== true) {
        setAutoMode(true)
        void window.desktop?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode: true, askConnectors, keepATodoList, replySize, layout, tube }).then((settings) => setAutoMode(settings.autoMode === true)).catch(() => setAutoMode(false))
      }
      setMode(answer.mode)
    }
    if (answer.teammate !== undefined) {
      const mate = teammates.find((one) => one.name.toLowerCase() === answer.teammate?.toLowerCase())
      if (mate !== undefined) setSelectedTeammateId(mate.teammateId)
    }
    setCommandAsk(undefined)
    setHandBack({ text: answer.text, attachments: [] })
  }
  const chooseOwnCommand = (name: string, source: 'global' | 'project', args: string): void => {
    const bridge = window.desktop?.ownCommands
    if (bridge === undefined) return
    void bridge.expand({ ...commandContext(), name, source, arguments: args }).then((answer) => {
      if (!answer.ok) return
      if ('ask' in answer) { setCommandAsk({ name, source, args, lines: answer.ask }); return }
      applyOwnCommand(answer)
    }).catch(() => undefined)
  }
  const allowCommandAsk = (): void => {
    const pending = commandAsk
    const bridge = window.desktop?.ownCommands
    if (pending === undefined || bridge === undefined) return
    void bridge.allow(commandContext()).then(async (allowed) => {
      if (!allowed.ok) return undefined
      return bridge.expand({ ...commandContext(), name: pending.name, source: pending.source, arguments: pending.args })
    }).then((answer) => {
      if (answer === undefined || !answer.ok || 'ask' in answer) return
      applyOwnCommand(answer)
    }).catch(() => undefined)
  }
  const runOwnCommand = (command: string): void => {
    const bridge = window.desktop?.ownCommands
    const key = draftKeyRef.current
    const localId = `local-${String(Date.now())}`
    const place = (entry: HeldCommand): void => {
      const next = commandEntriesRef.current.map((one) => one.id === localId ? entry : one)
      commandEntriesRef.current = next
      setCommandEntries(next)
    }
    const entry: HeldCommand = { id: localId, key, command, output: '', exitCode: null, stopped: false, running: true, carried: false }
    const added = [...commandEntriesRef.current, entry]
    commandEntriesRef.current = added
    setCommandEntries(added)
    if (bridge === undefined) { place({ ...entry, running: false, output: 'Locust is not ready yet. Nothing was run.', exitCode: 1 }); return }
    void bridge.run(commandContext(), command).then((answer) => {
      if (!answer.ok) { place({ ...entry, running: false, output: answer.message, exitCode: 1 }); return }
      const early = commandResults.current.get(answer.id)
      commandResults.current.delete(answer.id)
      place({
        ...entry,
        id: answer.id,
        ...(early === undefined ? {} : { output: early.output, exitCode: early.exitCode, stopped: early.stopped, running: false, ...(early.omitted === undefined ? {} : { omitted: early.omitted }) })
      })
    }).catch(() => place({ ...entry, running: false, output: 'That command could not be started. Nothing was run.', exitCode: 1 }))
  }
  useEffect(() => { setPagePoints([]) }, [composerDraftKey])
  const besideRow = besideId === undefined ? undefined : sidebarMissions.find((entry) => (entry.memberIds ?? [entry.missionId]).includes(besideId))
  const besideRun = ((): LiveRunState | undefined => {
    if (besideId === undefined) return undefined
    const newest = besideRow?.missionId ?? besideId
    const members = besideRow?.memberIds ?? [newest]
    // The one on screen already is not drawn twice.
    if (liveRun?.data?.missionId !== undefined && members.includes(liveRun.data.missionId)) return undefined
    const live = [...runs.values()].find((run) => run.data?.missionId === newest)
    if (live !== undefined) return live
    const held = historyById.get(newest)
    return held === undefined ? undefined : reopenedRun(held, historyById)
  })()
  // The right-click menu is built outside render and names the row it was
  // opened on, so it reads the rows through this.
  /*
   * The menu is built outside render, so it reads groups through refs for
   * the same reason it reads rows and teammates through them.
   */
  groupsRef.current = groups
  groupMembersRef.current = groupMembers
  sidebarMissionsRef.current = sidebarMissions
  teammatesRef.current = teammates
  routinesRef.current = routines
  historyByIdRef.current = historyById
  missionOwnersRef.current = missionOwners
  /*
   * Everything waiting on the person (needsYou.ts, 0.373): paused runs,
   * questions a teammate stopped on, memory suggestions. The title bar says
   * how many; its list opens where each is answered.
   */
  const needsYouItems = useMemo(
    () =>
      needsYou({
        approvals,
        ownerOfRun: (runId) => {
          const run = runs.get(runId)
          return run === undefined ? undefined : ownerOf(run)
        },
        conversations: sidebarMissions,
        ownerOfMission: (missionId) => missionOwners[missionId],
        eventsOf: (missionId) => {
          const live = [...runs.values()].find((run) => run.data?.missionId === missionId)
          if (live !== undefined) return live.events
          const recorded = historyById.get(missionId)
          return recorded === undefined || recorded.events.length === 0 ? undefined : recorded.events
        },
        nameOf: (teammateId) => teammates.find((entry) => entry.teammateId === teammateId)?.name,
        // A line suggested for About you waits on the same screen (0.424).
        memoryWaiting: memories.filter((memory) => memory.status === 'proposed').length + aboutYouSuggestions.length
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [approvals, runs, sidebarMissions, missionOwners, historyById, teammates, memories]
  )
  // The same count on the taskbar (0.379): a dot while anything needs you, a
  // flash when more does while the window is elsewhere (taskbar-attention.ts).
  // Missions with a live run here: the record has no word for running, so
  // every screen that draws a mission's state reads this (Missions, Team).
  const runningMissionIds = useMemo(
    () =>
      new Set(
        [...runs.values()]
          .filter((run) => liveRunIsActive(run) && run.data !== undefined)
          .map((run) => run.data!.missionId)
      ),
    [runs]
  )
  // The board's facts (0.585): which conversations wait on the person, and for what.
  const needsYouMissionIds = useMemo(() => new Set(needsYouItems.flatMap((item) => (item.kind === 'memory' ? [] : [item.missionId]))), [needsYouItems])
  const waitingFor = useMemo(() => new Map(needsYouItems.flatMap((item) => (item.kind === 'memory' ? [] : [[item.missionId, item.what] as const]))), [needsYouItems])
  // What ended while the person was away (0.590), from the rows already here; the tray gets the counts.
  /*
   * What a run that ended while the person was away came to, for its row on Home (0.758): why it failed, or its
   * answer's first line -- a night's polish says what it fixed before anyone opens it.
   */
  const awaySaid = (missionId: string): string | undefined => {
    const events = [...runs.values()].find((run) => run.data?.missionId === missionId)?.events ?? historyById.get(missionId)?.events
    if (events === undefined) return undefined
    const failed = [...events].reverse().find((event) => event.type === 'run.failed')
    if (failed !== undefined && failed.type === 'run.failed' && typeof failed.payload.message === 'string') {
      const why = failed.payload.message.split('\n').map((part) => part.trim()).find((part) => part.length > 0)
      if (why !== undefined) return why.length > 140 ? `${why.slice(0, 139).trimEnd()}…` : why
    }
    return lastSaid(events)
  }
  const awaySummary = useMemo(
    () => (awaySince === undefined ? undefined : sinceYouWereAway({ since: awaySince, rows: sidebarMissions, routines, needsYou: needsYouMissionIds })),
    [awaySince, sidebarMissions, routines, needsYouMissionIds]
  )
  useEffect(() => {
    window.desktop?.setAwaySummary?.(awaySummary === undefined ? null : awayCounts(awaySummary))
  }, [awaySummary])
  const awaySeen = (): void => {
    setAwaySince(undefined)
    window.desktop?.awaySeen?.()
  }
  const needsYouCount = needsYouItems.length
  useEffect(() => {
    window.desktop?.setNeedsYouCount?.(needsYouCount)
  }, [needsYouCount])
  // What is on screen, for the update handler to read (0.380): a finish the
  // person is looking at is not an unseen one. And looking clears it.
  lookingAtRef.current = { screen, teammateId: selectedTeammateId }
  useEffect(() => {
    if (screen !== 'workroom' || selectedTeammateId === undefined) return
    setFinishedUnseen((current) => {
      if (!current.has(selectedTeammateId)) return current
      const next = new Set(current)
      next.delete(selectedTeammateId)
      return next
    })
  }, [screen, selectedTeammateId])
  /*
   * "Wren finished" (0.379): a long run the person started, seen ending here.
   * Only the moment a live run BECOMES finished -- each run is told once --
   * and finishedToast.ts decides whether it is worth a toast at all; the host
   * says it only while the window is elsewhere.
   */
  const phaseSeen = useRef(new Map<string, LiveRunPhase>())
  useEffect(() => {
    for (const [key, run] of runs) {
      const before = phaseSeen.current.get(key)
      phaseSeen.current.set(key, run.phase)
      const wasLive = before === 'starting' || before === 'running' || before === 'cancelling'
      if (!wasLive || (run.phase !== 'completed' && run.phase !== 'failed')) continue
      const toast = finishedToast({
        phase: run.phase,
        startedAtIso: run.startedAtIso,
        endedAtMs: Date.now(),
        startedBy: run.startedBy,
        restored: run.restored === true,
        teammateName: run.teammateId === undefined ? undefined : teammates.find((entry) => entry.teammateId === run.teammateId)?.name,
        events: run.events,
        error: run.error
      })
      if (toast !== undefined) {
        window.desktop?.notifyFinished?.({ ...toast, ...(run.data?.missionId === undefined ? {} : { missionId: run.data.missionId }) })
      }
    }
  }, [runs, teammates])
  // Its toast, clicked, opens the conversation it finished in.
  const openMissionRef = useRef<(missionId: string) => void>(() => undefined)
  openMissionRef.current = openMission
  /*
   * THE CONVERSATION ON SCREEN FOLLOWS A TURN THAT CAME BACK FROM THE BACKGROUND (W10). Whenever the history or the
   * list of returned turns changes: a returned turn of THIS conversation -- the same first turn, walked back through
   * `continuesFrom` -- that is not on screen yet is opened, so it shows where it belongs. Timing-free on purpose:
   * the drive found the turn arriving before the conversation was opened as often as after.
   */
  const followedTurns = useRef(new Set<string>())
  useEffect(() => {
    const live = liveRunRef.current
    if (live === undefined || liveRunIsActive(live) || backgroundTurnIds.size === 0) return
    const firstOf = (id: string): string => {
      let at = id
      for (let step = 0; step < 500; step += 1) {
        const before = historyById.get(at)?.continuesFrom?.missionId
        if (before === undefined || before === at) return at
        at = before
      }
      return at
    }
    const shownIds = [live.data?.missionId, live.restoredMission?.missionId, ...(live.earlierTurns ?? []).map((turn) => turn.missionId)].filter((id): id is string => id !== undefined)
    if (shownIds.length === 0) return
    const shownFirst = firstOf(shownIds[0]!)
    const newer = [...backgroundTurnIds]
      .filter((id) => historyById.has(id) && !shownIds.includes(id) && !followedTurns.current.has(id) && firstOf(id) === shownFirst)
      .sort((left, right) => Date.parse(historyById.get(right)?.createdAt ?? '') - Date.parse(historyById.get(left)?.createdAt ?? ''))
    if (newer[0] === undefined) return
    // Once each: a person who then opens an earlier turn on purpose is not pulled forward again.
    for (const id of newer) followedTurns.current.add(id)
    openMission(newer[0])
    // eslint-disable-next-line react-hooks/exhaustive-deps -- follows the history, the returned turns and the conversation opened, not every render's new openMission
  }, [historyById, backgroundTurnIds, liveRun?.data?.missionId, liveRun?.restoredMission?.missionId])
  useEffect(() => window.desktop?.onAttentionOpenMission?.((missionId) => openMissionRef.current(missionId)), [])
  // The conversation on screen, by its root, is where notes on its diffs live.
  const notesMissionId = liveRun?.data?.missionId
  const notesRow = notesMissionId === undefined ? undefined : sidebarMissions.find((entry) => (entry.memberIds ?? [entry.missionId]).includes(notesMissionId))
  const notesKey = notesRow?.rootId ?? notesRow?.missionId ?? notesMissionId
  const shownNotes = notesKey === undefined ? [] : diffNotes.get(notesKey) ?? []
  const changeShownNotes = (change: (notes: readonly DiffNote[]) => readonly DiffNote[]): void => {
    if (notesKey === undefined) return
    setDiffNotes((current) => {
      const next = new Map(current)
      const changed = change(current.get(notesKey) ?? [])
      if (changed.length === 0) next.delete(notesKey)
      else next.set(notesKey, changed)
      return next
    })
  }
  const diffNotesPlace: DiffNotesPlace | undefined =
    notesKey === undefined
      ? undefined
      : {
          notes: shownNotes,
          add: (note) => changeShownNotes((notes) => withNote(notes, note)),
          remove: (key) => changeShownNotes((notes) => notes.filter((entry) => entry.key !== key)),
          who: pickedTeammate?.name ?? 'your teammate'
        }
  const openNeedsYou = (anchor: HTMLElement): void => {
    if (rowMenu?.anchor === anchor) {
      setRowMenu(undefined)
      return
    }
    const box = anchor.getBoundingClientRect()
    setRowMenuArmed(undefined)
    setRowMenu({
      x: box.left,
      y: box.bottom + 4,
      title: 'Waiting for you',
      anchor,
      items: needsYouItems.map((item) => ({
        label: needsYouLabel(item),
        onSelect: () => (item.kind === 'memory' ? setScreen('memory') : openMission(item.missionId))
      }))
    })
  }
  // What each teammate's live run is doing, from its events -- the same
  // function the thread's working line uses, so the two cannot disagree.
  const liveActivityByOwner: Record<string, LiveActivity> = {}
  for (const run of runs.values()) {
    const owner = ownerOf(run)
    if (owner !== undefined && liveRunIsActive(run)) liveActivityByOwner[owner] = liveActivityOf(run.events, true)
  }

  /**
   * What the conversation header's `⋯` offers, built where the things it
   * needs already are.
   *
   * Each entry is present only where it is TRUE, which is the rule the four
   * buttons kept and the menu inherits: there is nothing to explain about an
   * action with no material, so a review with nobody to ask and a routine
   * with nothing to save are absent rather than greyed. An empty list draws
   * no button at all.
   */
  const headerActions: ContextMenuItem[] = []
  /*
   * Open in terminal (0.387; components/OpenInTerminal.tsx): first in the ...
   * menu, and only there since the header became a row of icons (0.797).
   */
  const terminalMissionId = liveRun?.data?.missionId ?? liveRun?.restoredMission?.missionId
  const shownTerminal =
    liveRun === undefined
      ? undefined
      : terminalOffer({
          runtime: liveRun.data?.runtime ?? liveRun.runtime,
          model: liveRun.data?.model,
          missionId: terminalMissionId,
          running,
          // `missionOwner` is declared further down; the same lookup, here.
          teammateName: teammates.find((teammate) => teammate.teammateId === ownerOf(liveRun))?.name,
          antigravityCli
        })
  if (shownTerminal !== undefined && shownTerminal.disabled === undefined && terminalMissionId !== undefined) {
    const id = terminalMissionId
    const offer = shownTerminal
    headerActions.push({ label: `Open in terminal (${shownTerminal.runtimeName})`, onSelect: () => openConversationInTerminal(id, offer) })
  }
  /*
   * Ask on the side (0.461): where the conversation's runtime can copy its
   * session -- Claude Code, Codex, OpenCode -- and there is a turn to copy.
   */
  const sideRuntime = liveRun === undefined ? undefined : liveRun.data?.runtime ?? liveRun.runtime
  if (terminalMissionId !== undefined && (sideRuntime === 'claude' || sideRuntime === 'codex' || sideRuntime === 'opencode')) {
    const of = terminalMissionId
    const model = liveRun === undefined ? 'this model' : routeModelName(sideRuntime, liveRun.data?.model ?? 'account-default', resolvedModels.get(`${sideRuntime}:${liveRun.data?.model ?? 'account-default'}`))
    headerActions.push({ label: 'Ask on the side', onSelect: () => setSideChat((current) => (current?.of === of ? current : { of, runIds: [], model })) })
  }
  // Claude's sessions too (0.556): closed, the panel was otherwise only reached by picking Cloud again.
  if (cloudTasks.length > 0 || claudeCloud.length > 0) {
    headerActions.push({ label: 'Cloud tasks', onSelect: () => setCloudPanel(true) })
  }
  // Ask for a review (0.799): one row that opens the reviewers beside it (reviewMenu.ts).
  if (liveRun !== undefined && !running) {
    const asks = reviewMenuItem(
      reviewersFor(liveRun),
      reviewPairFor(liveRun),
      (teammate) => runtimeDisplayName(teammate.runtime as MissionRuntimeId),
      (reviewer) => askForReview(liveRun, reviewer),
      (pair) => askTwoForReview(liveRun, pair)
    )
    if (asks !== undefined) headerActions.push(asks)
  }
  if (saveAsRoutineId !== undefined) {
    headerActions.push({ label: 'Save conversation as routine', onSelect: () => openSaveRoutine(saveAsRoutineId) })
  }
  /*
   * SEND FEEDBACK about this conversation -- Claude Code's box (Colin: "for
   * bug reporting we can use what claude code does"), carrying what was said
   * here, which is what a report about it needs. Words only (feedback.ts).
   */
  if (liveRun !== undefined) {
    const shown = liveRun
    headerActions.push({
      label: 'Send feedback',
      onSelect: () => {
        const turns = [
          ...(shown.earlierTurns ?? []).map((turn) => ({ prompt: turn.prompt, events: turn.events })),
          { prompt: shown.prompt, events: shown.events }
        ]
        const runtime = shown.data?.runtime ?? shown.runtime ?? 'codex'
        const model = shown.data?.model
        const speaker = `${missionOwner?.name ?? 'The teammate'} (${runtimeDisplayName(runtime)}${model === undefined || model.length === 0 ? '' : `, ${modelDisplayName(runtime, model)}`})`
        setFeedbackFor({ conversation: conversationText(turns, speaker) })
      }
    })
  }
  if (!running && liveRun?.data?.missionId !== undefined) {
    const shownId = liveRun.data.missionId
    headerActions.push({
      label: 'Delete conversation',
      confirmLabel: 'Delete for good?',
      danger: true,
      onSelect: () => {
        setDeleteError(undefined)
        // Every turn of the conversation on screen, not just the one whose
        // id the header carries. Deleting the last turn and leaving the row
        // reads as the control doing nothing (Colin, 2026-09-05).
        const row = sidebarMissionsRef.current.find((entry) =>
          (entry.memberIds ?? [entry.missionId]).includes(shownId)
        )
        for (const turn of row?.memberIds ?? [shownId]) deleteMissionById(turn)
      }
    })
  }

  const noRuntimeReady =
    runtimeState.phase !== 'ready' || (!runtimes.some((runtime) => runtime.ready && runtime.status === 'ready') && !addedAccounts.some(a => a.state === 'ready'))
  /*
   * WHAT'S NEW, AND THE SPLASH.
   *
   * The home banner ("Locust 0.277.0 is running. Here is what changed.") is
   * gone -- Colin: "it adds a needless scrollbar on that title menu" -- and the
   * whole changelog lives in Settings as What's new. After an update, the
   * home screen says anything only when a build since the last one seen is
   * marked big ("if we have really good big updates where the user has to
   * know things, we can have a splash page on update"), and then once.
   *
   * Held to the banner's old conditions: Home (no conversation on screen)
   * with a runtime ready -- nothing interrupts a person talking, and a launch
   * whose runtimes are still signed out does not spend the one showing.
   */
  const [splashClosed, setSplashClosed] = useState(false)
  const [settingsLanding, setSettingsLanding] = useState<SettingsPageId | undefined>(undefined)
  const splashEntries = changelog?.firstRun === true ? (changelog.splash ?? []) : []
  const splashOpen = splashEntries.length > 0 && !splashClosed && screen === 'workroom' && !noRuntimeReady && shownKey === undefined
  // The landing is for the one opening "See every version" asked for.
  useEffect(() => {
    if (screen !== 'settings') setSettingsLanding(undefined)
  }, [screen])
  /*
   * The version is marked seen when the splash is actually on screen -- or,
   * on an update with nothing big, when Home first is -- and once. It was
   * once marked when the changelog was READ, at mount, so a banner held for
   * a runtime to connect was never shown after a relaunch either (Fable,
   * pass 1, finding 7).
   */
  const changelogMarked = useRef(false)
  useEffect(() => {
    if (changelogMarked.current) return
    if (screen !== 'workroom' || noRuntimeReady || changelog?.firstRun !== true) return
    if (splashEntries.length > 0 && !splashOpen) return
    changelogMarked.current = true
    void window.desktop?.markChangelogSeen().catch(() => undefined)
  }, [screen, noRuntimeReady, changelog, splashEntries.length, splashOpen])

  // Whose mission is on screen: the owner the host recorded, never the
  // composer's current target, which may already be someone else.
  const missionOwner =
    liveRun === undefined
      ? undefined
      : teammates.find((teammate) => teammate.teammateId === ownerOf(liveRun))
  const shownRunId = liveRun?.data?.runId
  /**
   * The one line under a mission's title on the Missions screen.
   *
   * Read from the same places the thread reads: the run's pending approval,
   * then its live line. Nothing invented for it, and nothing at all for a
   * mission that has settled -- a list where every row carries a sentence is
   * a list nobody scans.
   *
   * Grok Build's dashboard row does the same thing and says why in its own
   * source: the secondary line holds "the last tool call, the last assistant
   * message, or a 'Pending: ...' preview of the front-most permission
   * request". Twelve rows reading RUNNING tell you which to open only by
   * opening them.
   */
  const liveTurnOf = (missionId: string): LiveTurn | undefined => {
    const run = [...runs.values()].find((entry) => entry.data?.missionId === missionId)
    if (run === undefined || !liveRunIsActive(run)) return undefined
    const live = buildThread(run.events, { running: true, ...(run.startedAtIso === undefined ? {} : { startedAt: run.startedAtIso }), ...startingLabelOf(run) })
      .find((item) => item.type === 'live-step')
    if (live === undefined || live.type !== 'live-step') return undefined
    return {
      register: live.register,
      label: live.label,
      ...(live.action === undefined ? {} : { action: live.action }),
      detail: live.detail,
      startedAt: live.startedAt,
      // The same rule the thread uses: the dots mean waiting on the model
      // with nothing to show, which is a reasoning step or a gap between
      // steps -- not a tool that is visibly running.
      thinking: live.waiting === true || live.register === 'thinking'
    }
  }

  const missionDoing = (missionId: string): string | undefined => {
    const waiting = approvals.find((request) => request.missionId === missionId)
    // What it wants beats what it is doing: a run that stopped to ask is not
    // doing anything, and that is the row a person is looking for.
    if (waiting !== undefined) return `Pending: ${waiting.summary}`
    const live = liveTurnOf(missionId)
    if (live === undefined) return undefined
    const word = REGISTER_WORD[live.register]
    const said =
      live.label === undefined || /^(thinking|working|starting)$/i.test(live.label.trim()) ? undefined : live.label.trim()
    return [word, said, live.detail].filter((part) => part !== undefined && part.length > 0).join(' · ')
  }

  /**
   * Every mission the Missions screen should list: the recorded ones, and the
   * ones happening RIGHT NOW.
   *
   * MEASURED 2026-09-11 (`probe-missions-say-what-they-want`): for 49
   * consecutive samples a teammate was visibly working in the sidebar and
   * this screen was empty. The list is built from the ledger, and a mission
   * reaches the ledger when it is recovered -- so the one screen in the app
   * whose job is "what is going on" was the last place to hear about it.
   *
   * A live run is not a recovered mission and cannot pretend to be one: it
   * has no checkpoints, no integrity count and no terminal phase. What it
   * does have is everything a ROW needs -- who, what was asked, which route,
   * when it started -- so that is what is filled in, and the fields a row
   * never reads stay empty rather than being invented.
   */
  const missionsToList = useMemo((): readonly PublicRecoveredMission[] => {
    const recorded = new Set(history.map((mission) => mission.missionId))
    const live: PublicRecoveredMission[] = []
    for (const run of runs.values()) {
      const data = run.data
      if (data === undefined || !liveRunIsActive(run) || recorded.has(data.missionId)) continue
      // A side question lives in its panel, not in the list (R33), as in the sidebar; a judge in its comparison.
      if (run.startedBy?.kind === 'side' || isHiddenRun(run.startedBy, data.missionId)) continue
      const startedAt = run.startedAtIso ?? run.events[0]?.occurredAt ?? new Date().toISOString()
      live.push({
        missionId: data.missionId,
        runId: data.runId,
        workspaceId: workspaceId ?? '',
        prompt: run.prompt,
        runtime: data.runtime,
        model: data.model ?? 'account-default',
        requestedRouteId: data.resolvedRouteId,
        resolvedRouteId: data.resolvedRouteId,
        cliVersion: data.cliVersion ?? null,
        createdAt: startedAt,
        lastUpdatedAt: run.events.at(-1)?.occurredAt ?? startedAt,
        // A live run has not ended, and every value this field can take is an
        // ending. `interrupted` is the one that does not claim it finished;
        // the row's own tag comes from `runningMissionIds` regardless.
        phase: 'interrupted',
        events: run.events,
        eventCount: run.events.length,
        // At the cap, the live run holds a window of its events, and what is
        // counted from them is the window's; it says so, as the history does
        // (a B4 lead: this said nothing was cut).
        eventsTruncated: run.eventsTruncated === true,
        integrityIssueCount: 0,
        sandbox: data.sandbox,
        checkpoints: [],
        peerMessages: run.peerMessages ?? []
      })
    }
    // Most recently ACTIVE first, not most recently started: a conversation
    // that has been working for an hour belongs above one that opened five
    // minutes ago and has said nothing since. grok-build's `last_progress_at`,
    // and the reason it is their dashboard's sort key.
    return [
      ...live.sort((a, b) => lastActivityAt(b).localeCompare(lastActivityAt(a))),
      // Side questions (R33): "2 conversations" counted a question asked in a panel as one.
      ...history.filter((mission) => mission.startedBy?.kind !== 'side' && !hiddenRunIds.has(mission.missionId))
    ]
  }, [history, runs, workspaceId, hiddenRunIds])

  const shownApprovals = approvals.filter((request) => request.runId === shownRunId)
  // Paused runs AND questions a teammate stopped on: both make its face wait on you (0.564).
  const pendingApprovalsByOwner = waitingByTeammate(needsYouItems)
  const shownMissionId = liveRun?.data?.missionId ?? shownKey
  // Looking at a conversation takes its finish out of the board's Ready to look at (0.585), as looking at a teammate does above.
  lookingAtRef.current = { ...lookingAtRef.current, missionId: shownMissionId }
  useEffect(() => {
    if (screen !== 'workroom' || shownMissionId === undefined) return
    setFinishedUnseenMissions((current) => {
      const shown = sidebarMissions.find((row) => conversationKeys(row).includes(shownMissionId))
      const gone = shown === undefined ? [shownMissionId] : [shownMissionId, ...conversationKeys(shown)]
      if (!gone.some((id) => current.has(id))) return current
      const next = new Set(current)
      for (const id of gone) next.delete(id)
      return next
    })
  }, [screen, shownMissionId, sidebarMissions])
  const shownQuestionOpen = needsYouItems.some((item) => item.kind === 'decision' && item.missionId === shownMissionId)
  /*
   * WHAT WAS SAID ON CALLS, IN THE THREAD (0.747): the calls kept with the conversation on screen, read again after a
   * call is saved, and the call going on now when it belongs here.
   */
  const callsSaved = useCallsSaved()
  const callNow = useLiveCall()
  const conversationTurnIds = useMemo(
    () => liveRun === undefined ? [] : [...(liveRun.earlierTurns ?? []).map((turn) => turn.missionId), ...(liveRun.data?.missionId === undefined ? [] : [liveRun.data.missionId])],
    [liveRun]
  )
  const conversationTurnKey = conversationTurnIds.join(',')
  const [savedCalls, setSavedCalls] = useState<readonly SavedCall[]>([])
  useEffect(() => {
    if (conversationTurnIds.length === 0) { setSavedCalls([]); return }
    let alive = true
    void window.desktop?.calls?.list(conversationTurnIds).then((list) => { if (alive) setSavedCalls(list) }).catch(() => undefined)
    return () => { alive = false }
    // The key stands for the ids: a new array of the same turns is not a new conversation.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [conversationTurnKey, callsSaved])
  /*
   * YOUR CLOUD COMPUTER (0.761). The shown conversation's turns that ran there, read when it is opened; a turn
   * starting or ending there says so itself ('cloud-turn').
   */
  const shownTurnIds = useMemo(
    () => [...(liveRun?.earlierTurns ?? []).map((turn) => turn.missionId), ...(liveRun?.data?.missionId === undefined ? [] : [liveRun.data.missionId])],
    [liveRun?.earlierTurns, liveRun?.data?.missionId]
  )
  const shownTurnKey = shownTurnIds.join(',')
  useEffect(() => {
    if (shownTurnIds.length === 0) return
    void window.desktop?.cloudTurns(shownTurnIds).then((found) => {
      if (found.length === 0) return
      setCloudTurns((current) => {
        const next = new Map(current)
        for (const turn of found) next.set(turn.missionId, turn)
        return next
      })
    }).catch(() => undefined)
    // eslint-disable-next-line react-hooks/exhaustive-deps -- keyed by the ids themselves
  }, [shownTurnKey])
  const cloudTurnAct = async (missionId: string, act: 'apply' | 'discard'): Promise<string | undefined> => {
    const bridge = window.desktop
    if (bridge === undefined) return 'Locust is not ready yet.'
    const done = await (act === 'apply' ? bridge.applyCloudTurn(missionId) : bridge.discardCloudTurn(missionId)).catch(() => undefined)
    if (done === undefined) return 'Locust could not reach your cloud computer. Try again in a moment.'
    if (!done.ok) return done.message
    setCloudTurns((current) => new Map(current).set(missionId, done.turn))
    return undefined
  }
  /** The change waiting there, sent to a pull request (0.762): what went wrong, or undefined once it is there. */
  const cloudTurnPullRequest = async (missionId: string, message: string): Promise<string | undefined> => {
    const bridge = window.desktop
    if (bridge === undefined) return 'Locust is not ready yet.'
    const done = await bridge.pullRequestCloudTurn(missionId, message).catch(() => undefined)
    if (done === undefined) return 'Locust could not reach your cloud computer. Try again in a moment.'
    if (!done.ok) return done.message
    setCloudTurns((current) => new Map(current).set(missionId, done.turn))
    return undefined
  }
  const threadCloud = {
    turns: cloudTurns,
    onPatch: async (missionId: string) => window.desktop?.cloudTurnPatch(missionId).catch(() => undefined),
    onApply: (missionId: string) => cloudTurnAct(missionId, 'apply'),
    onDiscard: (missionId: string) => cloudTurnAct(missionId, 'discard'),
    onPullRequest: cloudTurnPullRequest,
    ...(pickedTeammate === undefined ? {} : { teammate: pickedTeammate.name })
  }
  /**
   * Cloud picked for an OpenCode teammate (0.761): the first time for a repository, the dialog asks. The cloud computer
   * picked for a Claude Code or Codex teammate (0.768): the same, and an agent not known to be signed in there is
   * offered the sign-in now, before anything is sent.
   */
  const checkCloudComputer = async (agent?: CloudAgent): Promise<void> => {
    const state = await window.desktop?.cloudComputerForFolder().catch(() => undefined)
    if (state === undefined) return
    if (state.ok) {
      setCloudSetup(
        state.computer === undefined
          ? { phase: 'ask', repo: state.repo, ...(agent === undefined ? {} : { agent }) }
          : agent !== undefined && state.computer.agents?.[agent] !== 'signed-in'
            ? { phase: 'sign-in', repo: state.repo, computer: state.computer.name, agent }
            : undefined
      )
      return
    }
    setCloudSetup(
      state.permission === true
        ? { phase: 'permission', message: state.message }
        : state.notOnGitHub === true
          ? { phase: 'not-on-github', message: state.message }
          : { phase: 'failed', message: state.message }
    )
  }
  const setUpCloudComputer = async (repo: string, agent?: CloudAgent): Promise<void> => {
    const withAgent = agent === undefined ? {} : { agent }
    setCloudSetup({ phase: 'working', repo, step: 'making', ...withAgent })
    const done = await window.desktop?.setUpCloudComputer(agent).catch(() => undefined)
    if (done === undefined) {
      setCloudSetup({ phase: 'failed', repo, message: 'Locust could not reach GitHub. Try again in a moment.' })
      return
    }
    if (done.ok) {
      setCloudSetup({ phase: 'ready', repo: done.repo, ...withAgent })
      return
    }
    setCloudSetup(done.permission === true ? { phase: 'permission', message: done.message } : { phase: 'failed', repo, message: done.message, ...withAgent })
  }
  /** Claude Code or Codex signed in on that cloud computer (0.768): its maker's page opens in the person's browser. */
  const signInCloudAgent = async (repo: string, computer: string, agent: CloudAgent): Promise<void> => {
    setCloudSetup({ phase: 'signing-in', repo, computer, agent, step: 'installing' })
    const done = await window.desktop?.signInCloudAgent(computer, agent).catch(() => undefined)
    if (done === undefined) {
      setCloudSetup({ phase: 'failed', repo, computer, agent, message: 'Locust could not reach your cloud computer. Try again in a moment.' })
      return
    }
    if (done.ok) {
      setCloudSetup((current) => ({ phase: 'signed-in', repo, agent, ...(current?.phase === 'signing-in' && current.woke === true ? { woke: true as const } : {}) }))
      return
    }
    setCloudSetup(done.permission === true ? { phase: 'permission', message: done.message } : { phase: 'failed', repo, computer, agent, message: done.message })
  }
  const threadCalls = useMemo(() => {
    const ids = new Set(conversationTurnIds)
    const here = callNow !== undefined && ((callNow.afterMissionId !== undefined && ids.has(callNow.afterMissionId)) || callNow.turns.some((turn) => ids.has(turn.missionId)))
    return [...savedCalls.filter((call) => call.callId !== callNow?.callId), ...(here && callNow !== undefined ? [{ ...callNow, live: callNow.phase !== 'ended' }] : [])]
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [savedCalls, callNow, conversationTurnKey])
  /*
   * HOW EACH OF A CALL'S TO-DOS IS GOING (0.755, callTodos.ts): the call on now's and those of the calls in the thread
   * on screen. Worked out on every render only while there are any, and handed down as a map that changes when a
   * to-do's state does -- not with every word a teammate streams.
   */
  const handOffsNow = useCallHandOffs()
  const callTodoIds = useMemo(
    () => [...new Set([...handOffsNow, ...threadCalls.flatMap((call) => call.handed ?? [])].map((handOff) => handOff.missionId))],
    [handOffsNow, threadCalls]
  )
  let callTodoKey = ''
  if (callTodoIds.length > 0) {
    const wanted = new Set(callTodoIds)
    const runPhase = new Map<string, string>()
    for (const [key, run] of runs) {
      const missionId = run.data?.missionId ?? key
      if (wanted.has(missionId)) runPhase.set(missionId, run.phase)
    }
    const waiting = new Set(needsYouItems.flatMap((item) => ('missionId' in item && typeof item.missionId === 'string' ? [item.missionId] : [])))
    callTodoKey = callTodoIds.map((id) => `${id}=${callTodoState({ ...(runPhase.has(id) ? { runPhase: runPhase.get(id)! } : {}), ...(historyById.get(id)?.phase === undefined ? {} : { recordPhase: historyById.get(id)!.phase }), needsYou: waiting.has(id) })}`).join(' ')
  }
  const callTodos = useMemo<CallTodos>(() => ({
    states: new Map(callTodoKey.length === 0 ? [] : callTodoKey.split(' ').map((pair) => pair.split('=') as [string, CallTodoState])),
    open: (missionId) => { openMissionRef.current(missionId) }
  }), [callTodoKey])
  // Which teammate is replaying a routine, and which step it is on. DERIVED
  // from the runs that are actually live rather than tracked alongside them:
  // a step that ended stops being reported because its run stopped running,
  // so the label can never outlive the work it describes.
  const routineStepByTeammate: Record<string, { readonly name: string; readonly step: number; readonly of: number }> = {}
  for (const run of runs.values()) {
    const startedBy = run.startedBy
    if (!liveRunIsActive(run) || startedBy?.kind !== 'routine' || run.teammateId === undefined) continue
    const routine = routines.find((entry) => entry.routineId === startedBy.routineId)
    if (routine === undefined) continue
    routineStepByTeammate[run.teammateId] = {
      name: routine.name,
      step: startedBy.step,
      of: routine.steps.length
    }
  }
  /*
   * And each teammate's state, decided once here with the same inputs the
   * sidebar uses, for the surfaces that do not compute their own.
   *
   * The WHOLE view, not just `.activity`. Keeping only the activity is what
   * left the Team roster with no presence dot at all -- not a roster that
   * chose not to draw one, a roster that was never handed the fact (Grok's
   * audit, 2026-09-13). A teammate blocked on a sign-in looked, on the one
   * screen that is entirely about teammates, exactly like a teammate with
   * nothing to do.
   */
  const viewByTeammate: Record<string, TeammateStatusView> = {}
  // When each teammate last worked (0.606): the newest of their conversations, for an idle card to say.
  const lastWorkedByTeammate: Record<string, string> = {}
  for (const teammate of teammates) {
    const owned = sidebarMissions.filter(
      (mission) => (mission.ownerId ?? missionOwners[mission.missionId]) === teammate.teammateId
    )
    const newest = owned.map((mission) => mission.lastAt).filter((at): at is string => at !== undefined).sort().at(-1)
    if (newest !== undefined) lastWorkedByTeammate[teammate.teammateId] = newest
    // One resolver for every surface; see `runtimeOfTeammate`.
    const theirRuntime = runtimeOfTeammate(teammate, owned)
    viewByTeammate[teammate.teammateId] = teammateStatusView({
      runtime: theirRuntime === undefined ? undefined : runtimes.find((entry) => entry.id === theirRuntime),
      ...runtimeReach(runtimes, runtimeState.phase === 'ready'),
      /*
       * Starting counts as working, because the sidebar says so.
       *
       * This map and the sidebar row called the SAME function with different
       * inputs, and so gave different answers for the same teammate. Grok
       * measured it in 0.116.0: title bar "1 running", sidebar Booty with a
       * green ring reading "working", and the Team card beside it with no
       * ring at all.
       *
       * I put that gap in. 0.109.0 gave the roster a presence dot and this
       * map to draw it from, and the map was a THIRD assembly of a fact that
       * already had two -- exactly the class Grok's first audit was about.
       * The sidebar consumes this map now rather than computing its own, so
       * there is one answer per teammate and the surfaces cannot disagree.
       */
      hasRunningMission:
        owned.some((mission) => mission.phase === 'running') || Object.keys(relayStarting).includes(teammate.teammateId),
      pendingApprovals: pendingApprovalsByOwner.get(teammate.teammateId) ?? 0,
      roleLabel: roleLabelOf(teammate),
      ...(liveActivityByOwner[teammate.teammateId] === undefined ? {} : { liveActivity: liveActivityByOwner[teammate.teammateId] }),
      recentlyDone: recentlyDone.includes(teammate.teammateId),
      recentlyReceived: recentlyReceived.includes(teammate.teammateId)
    })
  }

  /*
   * The workroom header's teammate, resolved ONCE.
   *
   * Both the face and the dot beside it are this teammate's state, so both
   * read it from here. They used to be assembled separately in the JSX and
   * had already drifted -- see the `presence` prop below.
   */
  const workroomOwnerView =
    missionOwner === undefined
      ? undefined
      : teammateStatusView({
          runtime: runtimes.find((entry) => entry.id === liveRun?.data?.runtime),
          ...runtimeReach(runtimes, runtimeState.phase === 'ready'),
          hasRunningMission: running,
          pendingApprovals: shownApprovals.length + (shownQuestionOpen ? 1 : 0),
          roleLabel: roleLabelOf(missionOwner),
          ...(liveActivityByOwner[missionOwner.teammateId] === undefined
            ? {}
            : { liveActivity: liveActivityByOwner[missionOwner.teammateId] }),
          recentlyDone: recentlyDone.includes(missionOwner.teammateId),
          recentlyReceived: recentlyReceived.includes(missionOwner.teammateId)
        })

  // Here, once every helper it reads is defined: computed above them it read one before its line had run.
  const benchmarkEntries = listBenchmarks()
  useEffect(() => {
    // A file or another panel explicitly opened takes this slot; the terminals keep their shells and screens.
    if (viewingFile !== undefined || viewerRefusal !== undefined || besideId !== undefined || reviewingId !== undefined || sideChat !== undefined || cloudPanel || backgroundPanel) {
      setTerminalOpen(false)
      if (browserOpen) closeBrowserPanel()
    }
  }, [viewingFile, viewerRefusal, besideId, reviewingId, sideChat, cloudPanel, backgroundPanel])
  /*
   * `has-inspector` is what insets the workroom so the drawer does not cover
   * the composer -- which cost a release to get right at 1120x720 (0.187.0).
   * The file viewer shares that region, so it shares the class: one inset
   * rule, whichever of the two is occupying the space.
   */
  const panelBesideOpen = (terminalOpen || browserOpen) && screen === 'workroom'
  return (
    <CallTodosContext.Provider value={callTodos}>
    <OrganizationPolicyContext.Provider value={build?.organizationPolicy}>
    <ApprovalRuleContext.Provider value={ruleOfCard}>
    <div className={`lc-shell${layoutMode === 'compact' ? ' is-compact' : ''}${panelBesideOpen ? ' has-viewer has-changes' : ''}${!panelBesideOpen && changesOpen && changesAvailable && screen === 'workroom' && viewingFile === undefined && viewerRefusal === undefined && besideRun === undefined && reviewing === undefined ? ' has-viewer has-changes' : ''}${!panelBesideOpen && (viewingFile !== undefined || viewerRefusal !== undefined || besideRun !== undefined || reviewing !== undefined) && screen === 'workroom' ? ' has-viewer' :!panelBesideOpen && inspectorOpen && liveRun !== undefined && screen === 'workroom' ? ' has-inspector' : ''}`}>
      <TitleBar
        // With no folder the composer chip already says so; the bar shows the
        // build instead (Colin, 2026-09-05).
        workspaceName={workspaceName.length === 0 ? `Locust${build === undefined ? '' : ` ${build.version}`}` : workspaceName}
        runningCount={runningCount}
        swarm={swarm}
        needsYou={needsYouItems.length}
        onNeedsYou={openNeedsYou}
        browser={liveRun === undefined ? <BrowserToggle open={browserOpen} refusal={browserRefusal} onToggle={toggleBrowser} /> : undefined}
        terminal={liveRun === undefined ? <TerminalToggle open={terminalOpen} refusal={terminalRefusal} onToggle={toggleTerminal} /> : undefined}
      />
      <div className="lc-body">
        {rowMenu !== undefined && (
          <ContextMenu
            state={rowMenu}
            armedLabel={rowMenuArmed}
            onArm={setRowMenuArmed}
            onClose={() => {
              setRowMenu(undefined)
              setRowMenuArmed(undefined)
            }}
          />
        )}
        <Sidebar
          runtimes={runtimes}
          routines={routines}
          onOpenAutomations={() => setScreen('automations')}
          onOpenBoard={() => setScreen('board')}
          {...(cloudTasks.length > 0 ? { cloudTasks: { count: cloudTasks.length, onOpen: () => { setScreen('workroom'); setCloudPanel(true) } } } : {})}
          {...(benchmarkEntries.length > 0 ? { benchmarks: { count: benchmarkEntries.length, onOpen: () => setScreen('benchmarks') } } : {})}
          {...(backgroundRuns.length > 0
            ? { background: { count: backgroundRuns.filter((run) => run.state === 'working' || run.state === 'blocked' || run.state === 'unknown').length, onOpen: () => { setScreen('workroom'); setCloudPanel(false); setBackgroundPanel(true) } } }
            : {})}
          missions={sidebarMissions}
          folders={folders}
          {...(workspaceId === undefined ? {} : { currentFolderId: workspaceId })}
          compact={layoutMode === 'compact'}
          teammates={teammates}
          routineStepByTeammate={routineStepByTeammate}
          branchByTeammate={Object.fromEntries((worktrees?.list ?? []).filter((tree) => tree.branch.length > 0).map((tree) => [tree.teammateId, tree.branch]))}
          missionOwners={missionOwners}
          selectedMissionId={liveRun?.data?.missionId ?? shownKey}
          // Highlight the pick, not the fallback: with nobody picked the
          // first teammate read as chosen while the composer addressed
          // nobody (user session, 2026-09-05).
          selectedTeammateId={pickedTeammate?.teammateId}
          onSelectMission={(missionId) => {
            // A comparison's row opens the comparison, until one of its columns is kept (0.441).
            const member = compareMembers.get(missionId)
            const compare = member === undefined ? undefined : compares.find((one) => one.compareId === member.compareId)
            openMission(missionId)
            if (compare !== undefined && compare.kept === undefined) setComparingId(compare.compareId)
          }}
          onMissionMenu={openMissionMenu}
          pinnedKeys={missionPins}
          settled={missionSettled}
          groups={groups}
          groupMembers={groupMembers}
          unreadableConversations={unreadableLedgers}
          unreadable={unreadableStores}
          onGroupMenu={openGroupMenu}
          renamingGroupId={renamingGroupId}
          onRenameGroup={(groupId, name) => {
            void window.desktop?.renameGroup(groupId, name).then(refreshGroups)
          }}
          onGroupRenameDone={() => setRenamingGroupId(undefined)}
          namingGroup={namingGroup}
          onAddMenu={openAddMenu}
          addMenuOpen={rowMenu?.anchor !== undefined}
          onNamingGroupDone={() => {
            setNamingGroup(false)
            setNewGroupFor(undefined)
          }}
          onNewGroup={(name) => {
            const waiting = newGroupFor
            setNewGroupFor(undefined)
            void window.desktop
              ?.createGroup(name)
              .then(async (made) => {
                /*
                 * The store answers with `{}` rather than the group, so the
                 * one just made is found by name from a fresh read. Names
                 * are not unique, so the NEWEST match wins -- making a
                 * second group called "Trading" must move the conversation
                 * into the one that was just created, not the older one.
                 */
                if (!made.ok || waiting === undefined) return
                const listed = await window.desktop?.listGroups()
                if (listed?.ok !== true) return
                const mine = listed.data.groups
                  .filter((group) => group.name === name.trim())
                  .sort((left, right) => Date.parse(right.createdAt) - Date.parse(left.createdAt))[0]
                if (mine !== undefined) await window.desktop?.assignGroup(waiting, mine.groupId)
              })
              .finally(refreshGroups)
          }}
          renamingMissionId={renamingMissionId}
          onRenameMission={(missionId, title) => void renameMission(missionId, title)}
          onRenameDone={() => setRenamingMissionId(undefined)}
          onTeammateMenu={openTeammateMenu}
          rooms={rooms}
          currentRoomId={screen === 'rooms' ? currentRoomId : undefined}
          screenKey={screen}
          onOpenRoom={(roomId) => {
            setRoomNotice(undefined)
            setCurrentRoomId(roomId)
            setScreen('rooms')
          }}
          onOpenRooms={openRoomsScreen}
          onHome={() => {
            setSelectedTeammateId(undefined)
            setShownKey(undefined)
            // A comparison too (0.569): one started with no teammate stayed on screen, so Home did nothing from it (Colin).
            setComparingId(undefined)
            setRoomNotice(undefined)
            setScreen('workroom')
          }}
          pendingApprovals={Object.fromEntries(pendingApprovalsByOwner)}
          liveActivity={liveActivityByOwner}
          viewByTeammate={viewByTeammate}
          starting={Object.keys(relayStarting)}
          recentlyDone={recentlyDone}
          recentlyReceived={recentlyReceived}
          handoffs={handoffs}
          onSelectTeammate={selectTeammate}
          onNewConversationWith={newConversationWith}
          onOpenHub={openHub}
          composerShown={screen === 'workroom'}
          onOpenSettings={() => {
            refreshWorktrees()
            refreshRuntimeSetup()
            const next = screen === 'settings' ? 'workroom' : 'settings'
            // Read the number when the screen that shows it opens, so it
            // cannot be the one from launch.
            if (next === 'settings') refreshStorage()
            setScreen(next)
          }}
          onOpenMissions={(teammateId) => {
            // A teammate's card asks for THEIR missions, and opens them even
            // when Missions is already open (L23); Ctrl 1 still toggles.
            setMissionsOwner(teammateId)
            if (teammateId !== undefined) setScreen('missions')
            else setScreen(screen === 'missions' ? 'workroom' : 'missions')
          }}
          onOpenTeammates={() => setScreen(screen === 'teammates' ? 'workroom' : 'teammates')}
        />
        <main className="lc-workroom">
          <OrganizationPolicyNotice />
          {screen === 'missions' ? (
            <MissionsScreen
              missions={missionsToList}
              unreadableLedgers={unreadableLedgers}
              {...(totalMissions === undefined ? {} : { totalMissions: totalMissions.total, listedMissions: totalMissions.listed })}
              ledgerUnreadable={ledgerUnreadable}
              workspaceId={workspaceId}
              runningMissionIds={runningMissionIds}
              titleOf={(mission) => missionTitle(typedPrompt(mission, historyById))}
              secondaryOf={missionDoing}
              /*
               * One at a time, through the same path a single delete takes.
               * The host refuses a live mission per id, so a batch that
               * happens to contain one loses that one and keeps the rest,
               * rather than the whole batch failing on its account.
               */
              onDeleteMissions={(missionIds) => {
                for (const missionId of missionIds) deleteMissionById(missionId)
              }}
              teammates={teammates}
              missionOwners={missionOwners}
              {...(missionsOwner === undefined ? {} : { ownerId: missionsOwner })}
              onShowEveryone={() => setMissionsOwner(undefined)}
              onOpen={openMission}
            />
          ) : screen === 'board' ? (
            <BoardScreen
              missions={sidebarMissions}
              rooms={rooms}
              // Deleted conversations never reach the sidebar's rows: the host keeps them for the trash screen alone.
              trashed={NO_TRASH}
              facts={{ needsYou: needsYouMissionIds, toLookAt: finishedUnseenMissions }}
              waitingFor={waitingFor}
              teammates={teammates}
              missionOwners={missionOwners}
              onOpen={openMission}
            />
          ) : screen === 'teammates' ? (
            <TeammatesScreen
              onShareTeam={() => setSharingTeam(true)}
              onAddTeamFromCard={() => addTeamFromCard()}
              {...(teamNotice === undefined ? {} : { teamNotice })}
              finishedUnseen={finishedUnseen}
              runningMissionIds={runningMissionIds}
              needsYou={needsYouItems}
              {...(routineNotice === undefined ? {} : { routineNotice })}
              onDismissRoutineNotice={() => setRoutineNotice(undefined)}
              teammates={teammates}
              missions={history}
              missionOwners={missionOwners}
              spendByTeammate={spendByTeammate}
              viewByTeammate={viewByTeammate}
              titleOf={(mission) => missionTitle(typedPrompt(mission, historyById))}
              onOpenMission={openMission}
              routines={routines}
              routineStepByTeammate={routineStepByTeammate}
              onRunRoutine={runRoutine}
              onRecoverRoutine={recoverRoutine}
              onRemoveRoutine={removeRoutine}
              onEditRoutine={editRoutine}
              onNewTeammate={() => {
                setTeammateError(undefined)
                setNewTeammateOpen(true)
              }}
              onEdit={(teammate) => {
                setTeammateError(undefined)
                setEditingTeammate(teammate)
              }}
              onRemove={removeTeammate}
              onMessage={openHub}
            />
          ) : screen === 'memory' ? (
            <MemoryScreen
              memories={memories}
              workspaceId={memoryWorkspace.id}
              workspaceName={memoryWorkspace.name}
              teammates={teammates}
              mode={memoryMode}
              onModeChange={changeMemoryMode}
              onAdd={addMemory}
              onUpdate={updateMemory}
              onRemove={removeMemory}
              onClear={clearMemories}
              forgotten={forgottenMemories}
              changedSince={memoriesOutOfDate}
              {...(briefTrackingSince === undefined ? {} : { briefTrackingSince })}
              onRestore={restoreMemory}
              onTidy={openTidyMenu}
              aboutYou={aboutYou}
              onSaveAboutYou={saveAboutYou}
              aboutYouSuggestions={aboutYouSuggestions}
              onAnswerAboutYou={answerAboutYou}
              // `openMission`, like every other opener. This had its own two
              // lines, and `setShownKey` wants a RUN key -- `runs` is keyed by
              // `run_...` -- so a `mission_...` id matched nothing and the
              // workroom drew its empty home screen with the previous teammate
              // still lit. Reproduced live by an outside QA, 2026-09-06.
              onOpenMission={openMission}
              notice={memoryNotice}
              noticeWaits={memoryNoticeWaits}
              onDismissNotice={() => setMemoryNotice(undefined)}
            />
          ) : screen === 'benchmarks' ? (
            <BenchmarksScreen entries={benchmarkEntries} onOpen={openBenchmark} />
          ) : screen === 'automations' ? (
            <AutomationsScreen
              onOpenMission={openMission}
              routines={routines}
              teammates={teammates}
              routineStepByTeammate={routineStepByTeammate}
              onRunRoutine={runRoutine}
              onRecoverRoutine={recoverRoutine}
              onEditRoutine={editRoutine}
              onRemoveRoutine={removeRoutine}
              onPauseRoutine={pauseRoutine}
              notice={routineNotice ?? automationNotice}
              onDismissNotice={() => { setRoutineNotice(undefined); setAutomationNotice(undefined) }}
              // The same rows the sidebar draws. An empty screen offers the
              // finished ones rather than describing how to save one.
              missions={sidebarMissions}
              folders={folders}
              onSaveRoutine={openSaveRoutine}
              onNewRoutine={openNewRoutine}
              onImportRoutine={previewRoutineImport}
              templates={routineTemplates}
              onUseTemplate={openRoutineTemplate}
              onBrowseTemplates={() => setTemplatePicker(true)}
              onExportRoutine={(id) => { void exportRoutine(id).catch((error: unknown) => setRoutineNotice(error instanceof Error ? error.message : 'That routine could not be exported. Check the destination folder and try again.')) }}
              onSettleRoutine={settleRoutine}
              signInOn={loginItem.openAtLogin}
              onOpenSignIn={() => {
                setSettingsLanding('app')
                setScreen('settings')
              }}
            />
          ) : screen === 'rooms' ? (
            <RoomScreen
              rooms={rooms}
              teammates={teammates}
              currentRoomId={currentRoomId}
              answersFor={roomAnswersFor}
              exchangeFor={roomExchangeFor}
              exchangeCostText={roomExchangeCostText}
              onSelectRoom={(roomId) => {
                setRoomNotice(undefined)
                setCurrentRoomId(roomId)
              }}
              onCreateRoom={createRoom}
              onRemoveRoom={removeRoom}
              onMenu={setRowMenu}
              onRenameRoom={renameRoom}
              onPost={postToRoom}
              onTask={updateRoomTask}
              onOpenMission={openMission}
              workspacePath={workspacePath}
              notice={roomNotice}
            />
          ) : screen === 'settings' ? (
            <SettingsScreen
              {...(scout === undefined ? {} : { scout })}
              {...((mate) => (mate?.route === undefined ? {} : { scoutModel: scoutModelWords(mate.route) }))(teammates.find((one) => one.teammateId === scout?.settings.teammateId))}
              {...(scoutModel === undefined ? {} : { scoutRoute: { runtime: 'opencode', model: scoutModel } })}
              {...((mate) => (mate === undefined ? {} : { onChangeScoutModel: () => { setEditingTeammate(mate) } }))(teammates.find((one) => one.teammateId === scout?.settings.teammateId))}
              workspacePath={workspacePath}
              workspaceMade={workspaceMade}
              onChooseFolder={chooseWorkspace}
              runtimes={runtimes}
              limitedRuntimes={limitedRuntimes}
              usageWindows={usageWindows}
              setupNotes={setupNotes}
              {...(runtimeUpdates === undefined ? {} : { runtimeUpdates })}
              onKeepAgentsCurrent={(automatic) => {
                void window.desktop
                  ?.setRuntimeUpdates(automatic)
                  .then(setRuntimeUpdates)
                  .catch(() => undefined)
              }}
              onInstall={installRuntime}
              {...(installing === undefined ? {} : { installing })}
              {...(installLineShown === undefined ? {} : { installLine: installLineShown })}
              {...(installFailure === undefined ? {} : { installFailure })}
              npmMissing={runtimeState.phase === 'ready' && runtimeState.npmPresent === false}
              onUpdateAgent={(runtime) => {
                void window.desktop
                  ?.updateRuntimeNow(runtime)
                  .then(setRuntimeUpdates)
                  .catch(() => undefined)
              }}
              runtimeSetup={runtimeSetup}
              cliArtifacts={cliArtifacts}
              workspaceBrief={workspaceBrief}
              worktrees={worktrees}
              onRemoveWorktree={removeWorktree}
              ledgerPath={undefined}
              build={build}
              storage={storage}
              update={update}
              onCheckUpdate={checkUpdate}
              onInstallUpdate={installUpdate}
              onUpdateLane={(everyBuild) => {
                void window.desktop
                  ?.setUpdateLane(everyBuild)
                  .then((response) => {
                    if (response.ok) setUpdate(response.data)
                  })
                  .catch(() => undefined)
              }}
              loginItem={loginItem}
              onLoginItemChange={(openAtLogin) => {
                void window.desktop?.setLoginItem(openAtLogin).then(setLoginItem).catch(() => undefined)
              }}
              onGeneralOpen={readStartup}
              keepRunning={keepRunning}
              onKeepRunningChange={(next) => {
                setKeepRunning(next)
                void window.desktop?.setKeepRunning(next).then((state) => setKeepRunning(state.keepRunning)).catch(() => setKeepRunning(!next))
              }}
              relay={relay}
              swarm={swarm}
              tube={tube}
            onTubeChange={chooseTube}
              replySize={replySize}
              onReplySizeChange={chooseReplySize}
              checkCommand={checkCommand}
              onCheckCommandSave={saveCheckCommand}
              metal={metal}
              metalStrength={metalStrength}
              metalMotion={metalMotion}
              metalBend={metalBend}
              onMetalChange={chooseMetal}
              terminalFaces={terminalFaces}
              onTerminalFacesChange={chooseTerminalFaces}
              plush={plush}
              onPlushChange={choosePlush}
            onSwarmChange={(next) => {
                // The same write the composer mark performs: optimistic, then
                // reconciled with what the store actually saved.
                setSwarm(next)
                if (next) setSwarmCalls((count) => count + 1)
                void window.desktop
                  ?.writeWorkspaceSettings({ swarm: next, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube })
                  .then((settings) => setSwarm(settings.swarm === true))
                  .catch(() => setSwarm(!next))
              }}
              autoMode={autoMode}
              onAutoModeChange={(next) => {
                setAutoMode(next)
                void window.desktop
                  ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode: next, askConnectors, keepATodoList, replySize, layout, tube })
                  .then((settings) => setAutoMode(settings.autoMode === true))
                  .catch(() => setAutoMode(!next))
              }}
              {...(planRunsOut === undefined ? {} : { planRunsOut })}
              carryOnChoices={carryOnChoices}
              onPlanRunsOutChange={(next) => {
                const before = planRunsOut
                setPlanRunsOut(next ?? undefined)
                void window.desktop
                  ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube, planRunsOut: next })
                  .then((settings) => setPlanRunsOut(settings.planRunsOut ?? undefined))
                  .catch(() => setPlanRunsOut(before))
              }}
              askConnectors={askConnectors}
              onOwnModelsChanged={readModels}
              onAskConnectorsChange={(next) => {
                setAskConnectors(next)
                void window.desktop
                  ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors: next, keepATodoList, replySize, layout, tube })
                  .then((settings) => setAskConnectors(settings.askConnectors === true))
                  .catch(() => setAskConnectors(!next))
              }}
              keepATodoList={keepATodoList}
              onKeepATodoListChange={(next) => {
                setKeepATodoList(next)
                void window.desktop
                  ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList: next, replySize, layout, tube })
                  .then((settings) => setKeepATodoList(settings.keepATodoList === true))
                  .catch(() => setKeepATodoList(!next))
              }}
              claudeOwnSkills={claudeOwnSkills}
              onClaudeOwnSkillsChange={(next) => {
                setClaudeOwnSkills(next)
                void window.desktop
                  ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube, claudeOwnSkills: next })
                  .then((settings) => setClaudeOwnSkills(settings.claudeOwnSkills === true))
                  .catch(() => setClaudeOwnSkills(!next))
              }}
              relayHopCap={relayHopCap}
              memoryMode={memoryMode}
              onMemoryModeChange={changeMemoryMode}
              layout={layout}
              layoutMode={layoutMode}
              onLayoutChange={chooseLayout}
              memoryCount={memories.filter((memory) => memory.status === 'kept').length}
              memoryWaiting={memories.filter((memory) => memory.status === 'proposed').length}
              onOpenMemory={() => setScreen('memory')}
              onRelayHopCapChange={(next) => {
                setRelayHopCap(next)
                void window.desktop
                  ?.writeWorkspaceSettings({ swarm, relay, relayHopCap: next, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube })
                  .then((settings) => setRelayHopCap(settings.relayHopCap))
                  .catch(() => undefined)
              }}
            onRelayChange={(next) => {
              setRelay(next)
              void window.desktop
                ?.writeWorkspaceSettings({ swarm, relay: next, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube })
                .then((settings) => setRelay(settings.relay === true))
                .catch(() => setRelay(!next))
            }}
            interrupt={interrupt}
            onInterruptChange={(next) => {
              setInterrupt(next)
              void window.desktop
                ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt: next, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube })
                .then((settings) => setInterrupt(settings.interrupt === true))
                .catch(() => setInterrupt(!next))
            }}
            onPreviewPrune={previewPrune}
            onListTrash={listTrash}
            onRestoreMission={restoreMission}
            onEmptyTrash={emptyTrash}
            changelog={changelog}
            {...(settingsLanding === undefined ? {} : { initialPage: settingsLanding })}
              onPrune={prune}
              onPickProfileFolder={pickProfileFolder}
              onBackUpProfile={backUpProfile}
              onPreviewProfileRestore={previewProfileRestore}
              onRestoreProfile={restoreProfile}
            />
          ) : comparing !== undefined ? (
            (() => {
              const { prompts, columns } = compareColumnsFor(comparing)
              return (
                <CompareView
                  onOpenFile={openFileInViewer}
                  compare={comparing}
                  changeLines={compareChangeLines?.compareId === comparing.compareId ? compareChangeLines.columns : {}}
                  prompts={prompts}
                  columns={columns}
                  owner={teammates.find((mate) => mate.teammateId === comparing.teammateId)}
                  onUseInstructions={useTunedInstructions}
                  workspacePath={workspacePath}
                  keeping={keepingCompare}
                  conversationMode={mode}
                  retrying={retryingCompare}
                  {...(compareProblem === undefined ? {} : { problem: compareProblem })}
                  onKeep={(slot) => void keepCompareColumn(comparing, slot)}
                  onRetry={(slot) => void retryCompareColumn(comparing, slot)}
                  decidingIds={decidingIds}
                  onDecide={decideApproval}
                  onAnswer={answerQuestion}
                  onBack={comparing.rerunOf !== undefined ? () => { setRevealBenchmarkOf(comparing.rerunOf); setComparingId(comparing.rerunOf) }
                    : backToBenchmarks ? () => { setComparingId(undefined); setScreen('benchmarks') }
                    : comparing.kept === undefined ? undefined : () => setComparingId(undefined)}
                  {...(comparing.rerunOf !== undefined ? { backLabel: 'Back to the benchmark' } : backToBenchmarks ? { backLabel: 'Back to Benchmarks' } : {})}
                  {...(() => {
                    const benchmark = benchmarkOf(comparing)
                    return benchmark === undefined ? {} : { benchmark: revealBenchmarkOf === comparing.compareId ? { ...benchmark, reveal: true } : benchmark }
                  })()}
                  // A judge's view, and the models one can be (0.520, after Optima).
                  {...(() => {
                    const judge = judgeViewOf(comparing)
                    const choices = judgeChoicesFor(comparing)
                    // Another model: only ones not already in it (0.523).
                    const compared = new Set(comparing.slots.map((column) => `${column.route.runtime}:${column.route.model}`))
                    const addable = choices.filter((one) => !compared.has(one.key))
                    return {
                      addChoices: addable,
                      adding: addingCompareModel,
                      onAddModel: (key: string) => {
                        const choice = addable.find((one) => one.key === key)
                        if (choice !== undefined) void addCompareModel(comparing, choice)
                      },
                      ...(judge === undefined ? {} : { judge }),
                      judgeChoices: choices,
                      judging: judgingCompare,
                      // Draft the points (0.766): one short run on the model picked, its points back for the box.
                      onDraftPoints: async (key: string): Promise<readonly string[] | string> => {
                        const choice = choices.find((one) => one.key === key)
                        const bridge = window.desktop
                        if (choice === undefined || bridge === undefined) return 'Pick a model to draft them.'
                        const answer = await bridge.draftJudgePoints({ compareId: comparing.compareId, route: { runtime: choice.runtime, model: choice.model, label: choice.label } }).catch(() => undefined)
                        return answer === undefined ? 'The points could not be drafted. Write them yourself.' : answer.ok ? answer.points : answer.message
                      },
                      onJudge: (key: string, criteria: string, others: readonly string[] = []) => {
                        const choice = choices.find((one) => one.key === key)
                        const panel = others.flatMap((other) => choices.filter((one) => one.key === other))
                        if (choice !== undefined && panel.length === others.length) void judgeCompare(comparing, choice, criteria, panel)
                      }
                    }
                  })()}
                  // Kept: the record across every decided comparison, as Optima's results table (0.519).
                  {...(comparing.kept === undefined ? {} : {
                    record: compareRecordRows(
                      compares,
                      (compare, slot) => {
                        const runs = (compare.slots.find((column) => column.slot === slot)?.missionIds ?? []).flatMap((id) => historyById.get(id) ?? [])
                        const ms = runs.reduce((total, run) => total + (runSpanMs(run.events) ?? 0), 0)
                        const cost = sumCosts(runs.map((run) => missionCost(run)))
                        return { ...(ms > 0 ? { ms } : {}), ...(cost === undefined ? {} : { cost }) }
                      },
                      (route) => route.label ?? route.model
                    )
                  })}
                />
              )
            })()
          ) : liveRun === undefined ? (
            // A CHOSEN teammate with nothing running gets their own
            // capability-led state. Nothing chosen -- which is how every
            // launch begins -- shows the home screen: what is connected,
            // which folder, and the sidebar to pick a teammate from (Colin,
            // 2026-09-05: "have that on open so the user can see if their
            // models are connected and then can click onto their teammates").
            // `selectedTeammate` falls back to the first teammate so a message
            // always has someone to go to; the SCREEN keys on the explicit pick.
            selectedTeammateId !== undefined && selectedTeammate !== undefined && !noRuntimeReady ? (
              <IdleTeammate
                teammate={pickedTeammate ?? selectedTeammate ?? teammates[0]!}
                canStart={busyRun === undefined}
                /*
                 * Whether THIS teammate can actually start, which this screen
                 * did not know at all.
                 *
                 * `canStart` above is only "no mission is running"; the screen
                 * was drawn whenever ANY runtime was ready. So a Claude
                 * teammate on a machine where Claude is signed out got the
                 * ordinary welcome and a row of starter buttons that could
                 * not work, while Settings said the opposite in red two
                 * clicks away (Astra's Finding 1, 2026-09-14).
                 */
                blocked={(() => {
                  const who = pickedTeammate ?? selectedTeammate ?? teammates[0]!
                  if (who.route?.account !== undefined) return addedAccounts.some(a => a.id === who.route?.account && a.state === 'ready') ? undefined : `Sign in to ${accountName(who.route.account)} in Settings > AI agents before ${who.name} starts.`
                  const view = viewByTeammate[who.teammateId]
                  if (view?.status !== 'blocked') return undefined
                  const theirs = runtimeOfTeammate(
                    who,
                    sidebarMissions.filter(
                      (mission) => (mission.ownerId ?? missionOwners[mission.missionId]) === who.teammateId
                    )
                  )
                  const command = theirs === undefined ? undefined : signInCommand(theirs)
                  const named =
                    theirs === undefined || !isMissionRuntime(theirs) ? 'Their AI agent' : runtimeDisplayName(theirs)
                  return command === undefined
                    ? `${named} is not signed in, so ${who.name} cannot start yet.`
                    : `${named} is not signed in, so ${who.name} cannot start yet. Sign in from Settings > AI agents, or ${command} in a terminal.`
                })()}
                mode={mode}
                onStarter={(prompt) => {
                  void startMission(prompt)
                }}
              />
            ) : (
              /*
               * The boot screen is NOT here any more.
               *
               * It is its own window now -- a loading screen shown before
               * this one exists, closing when the runtimes have answered
               * (`SplashApp`). Drawing it in the pane as well meant it
               * "immediately appeared and then disappeared" behind the
               * splash, which is the app showing the same ceremony twice
               * and finishing it in neither.
               */

              <FirstLaunch
                {...(awaySummary === undefined ? {} : { away: awaySummary, awayTeammates: teammates, onOpenAway: openMission, awaySaid, onOpenRoutines: () => setScreen('automations'), onAwaySeen: awaySeen })}
                {...((scoutShown) => (scoutShown === undefined ? {} : { scout: scoutShown }))((() => {
                  // Scout on Home (0.756): its cards when it is on; when it is not, one invitation for a person with a
                  // team, a folder and a free model to read with -- until they say Not now.
                  if (runtimeState.phase !== 'ready' || scout === undefined || teammates.length === 0) return undefined
                  if (scout.settings.on) {
                    // Nothing needs the person: one quiet line that says it keeps watch, not nothing at all (0.768).
                    // On its own team card when it is on the team (0.799, HomeTeam's `watch`); the line only if it is not.
                    if (scout.cards.length === 0) {
                      const onTeam = teammates.some((one) => one.teammateId === scout.settings.teammateId)
                      return onTeam ? undefined : <ScoutWatching state={scout} scout={undefined} onLookNow={lookWithScout} looking={scoutLooking} />
                    }
                    return <ScoutCards state={scout} scout={teammates.find((one) => one.teammateId === scout.settings.teammateId)} onOpen={openScoutCard}
                      onSend={sendScoutDraft} onDraftIntoBox={draftScoutIntoBox}
                      onDismiss={(cardId) => { void window.desktop?.scout?.dismiss(cardId).catch(() => undefined) }} />
                  }
                  // Met once is enough: a Scout turned off, or taken off the team, is turned on again from Settings.
                  if (scout.settings.teammateId !== undefined || scoutNotNow || scoutModel === undefined || workspacePath === undefined) return undefined
                  // Offered as a card on the team (teamInvite below); its words open over Home and take no row (0.808).
                  if (!scoutInviteOpen) return undefined
                  return <ScoutInvite model={scoutModelWords({ runtime: 'opencode', model: scoutModel })} onTurnOn={() => { void turnOnScout() }} onNotNow={scoutNotNowPressed}
                    onClose={() => { setScoutInviteOpen(false) }} busy={scoutBusy} {...(scoutMessage === undefined ? {} : { message: scoutMessage })} />
                })())}
                {...(() => {
                  // The same condition as the offer above: a team, a folder, a free model, and Scout never decided.
                  if (runtimeState.phase !== 'ready' || scout === undefined || teammates.length === 0 || scout.settings.on) return {}
                  if (scout.settings.teammateId !== undefined || scoutNotNow || scoutModel === undefined || workspacePath === undefined) return {}
                  return { teamInvite: { onOpen: () => { setScoutInviteOpen(true) }, onNotNow: scoutNotNowPressed } }
                })()}
                {...(() => {
                  // Scout keeping watch with nothing to say: its line on its own team card (0.799).
                  const scoutId = scout?.settings.on === true && scout.cards.length === 0 ? scout.settings.teammateId : undefined
                  if (scout === undefined || scoutId === undefined || !teammates.some((one) => one.teammateId === scoutId)) return {}
                  return { teamWatch: { teammateId: scoutId, line: <ScoutLooked state={scout} looking={scoutLooking} />, looking: scoutLooking, onLookNow: lookWithScout } }
                })()}
                onAddTeamFromCard={() => addTeamFromCard(true)}
                rosterUnreadable={unreadableStores.includes('teammates')}
                runtimes={runtimes}
                freeStart={freeStartStillFree(runtimes, models)}
                {...((() => {
                  // The free model that last answered here, else the catalogue's first free entry (0.514, 0.517).
                  if (route.runtime === 'opencode') return {}
                  const free = freeStartModel('opencode', models, freeAnswered, freeDown)
                  return free === undefined ? {} : { onUseFree: () => changeRoute({ runtime: 'opencode', model: free }) }
                })())}
                limitedRuntimes={limitedRuntimes}
                usageWindows={usageWindows}
                {...(window.desktop === undefined ? {} : {
                  readUsage: (range: UsageRange) => window.desktop!.readUsage(range),
                  // The record moved: a turn ended, or one was removed.
                  usageKey: `${String(history.length)}:${history[0]?.lastUpdatedAt ?? ''}`,
                  onOpenUsage: () => setUsageOpen(true)
                })}
                discoveryPhase={runtimeState.phase}
                tube={tube}
                swarmCalls={swarmCalls}
                coverActivity={coverWork}
                // Who is working and who waits on you, for the cover's glass (0.610): the same facts the cards say.
                {...((status) => (status === undefined ? {} : { coverStatus: status }))(runtimeState.phase !== 'ready' ? undefined : glassStatus(
                  teammates.filter((mate) => viewByTeammate[mate.teammateId]?.status === 'working').map((mate) => mate.name),
                  teammates.filter((mate) => viewByTeammate[mate.teammateId]?.status === 'approval-needed').map((mate) => mate.name)
                ))}
                workspacePath={workspacePath}
                workspaceMade={workspaceMade}
                teammateCount={teammates.length}
                team={teammates.map((mate) => ({
                  teammateId: mate.teammateId,
                  name: mate.name,
                  hue: mate.hue,
                  avatar: mate.avatar,
                  role: roleLabelOf(mate),
                  ...(mate.route === undefined ? {} : homeRouteOf(mate.route, resolvedModels, models)),
                  working: [...runs.values()].some((run) => liveRunIsActive(run) && ownerOf(run) === mate.teammateId),
                  // Where they stand, beside the name (0.606): the sidebar face's own fact, worded for the card. Idle says nothing (0.609); what they last did is the card's hover.
                  // And nothing until the agents have been looked for: before that every teammate reads as blocked.
                  ...((state) => (state === undefined ? {} : { state }))(runtimeState.phase === 'ready' ? teamCardState(viewByTeammate[mate.teammateId]) : undefined),
                  lastWorked: lastWorkedPhrase(lastWorkedByTeammate[mate.teammateId]),
                  // What they are doing, for the line under the name while they do it (0.610): the question they wait
                  // on, else the step under way in their running turn.
                  ...((doing) => (doing === undefined ? {} : { doing }))((() => {
                    const waiting = needsYouItems.find((item) => item.kind !== 'memory' && item.teammateId === mate.teammateId)
                    if (waiting !== undefined) return waitingLine(waiting)
                    const run = [...runs.values()].find((entry) => liveRunIsActive(entry) && ownerOf(entry) === mate.teammateId && entry.data !== undefined)
                    return run?.data === undefined ? undefined : liveTurnOf(run.data.missionId)?.action
                  })())
                }))}
                onMessageTeammate={selectTeammate}
                onChooseFolder={chooseWorkspace}
                onNewTeammate={() => {
                  setTeammateError(undefined)
                  setNewTeammateOpen(true)
                }}
                onUseTemplate={makeTeamFrom}
                onInstall={installRuntime}
                installing={installing}
                installLog={installLog}
                installLine={installLineShown}
                installFailure={installFailure}
                npmMissing={runtimeState.phase === 'ready' && runtimeState.npmPresent === false}
                npmIsBundled={runtimeState.phase === 'ready' && runtimeState.npmIsBundled === true}
                npmDidNotAnswer={runtimeState.phase === 'ready' && runtimeState.npmDidNotAnswer === true}
                checkingGaveUp={runtimeState.phase === 'ready' && runtimeState.gaveUp === true}
                onCompare={() => {
                  setCompareOn(true)
                  setPickerRequest((count) => count + 1)
                }}
                onStarter={(starter) => {
                  // The teammate on screen, in the mode the starter suits; the words wait in the box (StarterList.tsx).
                  setCompareOn(false)
                  if (starter.mode !== undefined) setMode(starter.mode)
                  setComposerFill((current) => ({ text: starter.prompt, seq: (current?.seq ?? 0) + 1 }))
                }}
                {...(githubReady ? { onStartFromIssue: openIssueList } : {})}
                onCompareStarter={(starter) => {
                  setCompareOn(true)
                  // The work (each model in its own copy) or the answers, as the starter asks.
                  setCompareChanges(starter.compareChanges)
                  setComposerFill((current) => ({ text: starter.prompt, seq: (current?.seq ?? 0) + 1 }))
                  // Two models already chosen, each its own dropdown (0.460, as
                  // Arena): the words and the models are in the box, and Send
                  // is the next press. Only with fewer than two to offer does
                  // the picker open.
                  const starting = comparePicks.length >= MIN_COMPARE_SLOTS ? comparePicks : startingPicks(true)
                  if (starting.length >= MIN_COMPARE_SLOTS) setComparePicks(starting)
                  else setPickerRequest((count) => count + 1)
                }}
                onCheckAgain={() => {
                  // The repair the app can actually perform: ask again, from
                  // the top, with CHECKING honest once more while it does.
                  resetDiscoveryBudget.current()
                  setRuntimeState((held) => (held.phase === 'ready' ? { ...held, gaveUp: false } : held))
                  // Everything, including whether npm is there: this is the
                  // repair, so nothing it could fix should be remembered.
                  askDiscoveryAgain.current(true)
                }}
              />
            )
          ) : (
            <>
              <header className="lc-workroom__header">
                <div className="lc-workroom__identity">
                  {missionOwner === undefined ? (
                    <AgentAvatar size={32} />
                  ) : (
                    <TeammateBot
                      hue={missionOwner.hue}
                      avatar={missionOwner.avatar}
                      size={BOT_SIZE.workroomHeader}
                      teammateId={missionOwner.teammateId}
                      activity={workroomOwnerView?.activity ?? 'idle'}
                      hopsWhenDone={false}
                      // The face you are typing to: it listens (faceLife.ts).
                      hears
                      /*
                       * The dot comes from the same status as the face.
                       *
                       * It used to be a ternary written out here -- running,
                       * else approvals, else none -- which is three ways of
                       * saying what `facePresenceFor` already says, and it
                       * disagreed in a way nobody would see until it mattered:
                       * it had no `blocked` branch AT ALL. A teammate whose
                       * runtime needed a sign-in drew the same nothing as a
                       * teammate with no work, two inches from a sidebar row
                       * drawing the red one correctly (Grok's audit,
                       * 2026-09-13). One call now decides both.
                       */
                      presence={workroomOwnerView === undefined ? 'none' : facePresenceFor(workroomOwnerView.status)}
                    />
                  )}
                  <div style={{ minWidth: 0 }}>
                    <div className="lc-workroom__titleline">
                      <span className="lc-workroom__name">
                        {missionOwner?.name ?? missionTitle(shownPrompt(liveRun))}
                      </span>
                      <span className="lc-workroom__role">
                        {missionOwner === undefined ? '' : `${roleLabelOf(missionOwner)} · `}
                        {/* A model of your own is said as yours; the line under this names it (routeChrome, 0.361). */}
                        {isOwnRoute(liveRun.data?.model ?? '') ? (
                          'Your model'
                        ) : (
                          <>
                            <RuntimeMark runtime={liveRun.data?.runtime ?? liveRun.runtime ?? 'codex'} size={12} className="is-inline" />
                            {runtimeDisplayName(liveRun.data?.runtime ?? liveRun.runtime ?? 'codex')}
                          </>
                        )}
                      </span>
                    </div>
                    <div className="lc-workroom__mission">
                      {liveRun.data === undefined
                        ? isTerminal(liveRun.phase)
                          // A start that failed has no mission id and never
                          // will. Leaving "Starting…" over a red error card
                          // said the opposite of what happened.
                          ? `Not started · ${liveRun.phase}`
                          // Not "Starting…": the thread already says
                          // "Starting · 3s" five hundred pixels below, and
                          // two places stating one fact is how they come to
                          // disagree. The thread's is the one that stays --
                          // it carries the clock (design agent, 2026-09-10).
                          : 'New conversation'
                        // The MODEL, not just the runtime. This app exists to
                        // put two models on the same work, and the header
                        // named only the runtime -- so two missions from
                        // different models read identically once the composer
                        // had moved on (design pass, 2026-09-04, asked for a
                        // provenance strip; this is the half of it that was
                        // actually missing). Ledger state was the other half
                        // and is NOT here: the receipt card below already
                        // states it, and two places stating one fact is how
                        // they come to disagree.
                        // Spelled as a name rather than an identifier, the
                        // same way the composer's chip spells it. The exact
                        // id is in the receipt below, which is where a person
                        // goes for a string to copy.
                        /*
                         * FOUR FACTS, ONE SCOPE, ONE TENSE.
                         *
                         * Design ruling, 2026-09-14 (`THREE-PLACEMENTS`),
                         * after this line truncated on Colin's screen as
                         * "… 208 out · co…". It had grown to seven facts
                         * because it is the most VISIBLE surface, not the
                         * right one -- "the usual reason a strip grows".
                         *
                         * Three things came off. The noun `Mission ·`, which
                         * labels the line once for a first-time reader and
                         * costs eight characters on every render after: an
                         * eight-hex id in mono in a header is self-evidently
                         * an id. The permission sentence, which went NOWHERE
                         * -- the composer's mode chip already names that mode
                         * 300px below, where it is actionable, and "a
                         * permission is a thing you can change, so stating it
                         * where it cannot be changed is the app talking to
                         * itself". And the conversation's total, to the
                         * context ring, which is already the
                         * conversation-scoped object drawing a
                         * conversation-scoped budget.
                         *
                         * Measured by the design pass off its own render at
                         * the inspector-open width: 884px of line in 676px of
                         * column, down to 363px.
                         */
                        /*
                         * THE MODEL, AND NEWS. (First-impressions pass, after
                         * 0.349; Colin, 2026-09-26: Claude's UI "and beyond"
                         * is the standard.) The eight-hex id and "restored
                         * from the local ledger" were an engineer's facts on
                         * the first line a person reads: the id is in
                         * Details, and a conversation that finished normally
                         * says nothing about it -- only running, failed,
                         * cancelled or interrupted is news. The model stays:
                         * which model did the work is the thing this app
                         * exists to keep legible.
                         */
                        : `${
                            // The account's own default says nothing the runtime beside the name does not (declutter, 0.723).
                            liveRun.data.model === undefined || liveRun.data.model === 'account-default'
                              ? ''
                              : // The same name the chip gives it: "Sonnet 5.5", not the alias's bare "Sonnet" (0.447).
                                routeModelName(liveRun.data.runtime, liveRun.data.model, resolvedModels.get(`${liveRun.data.runtime}:${liveRun.data.model}`))
                          }${
                            running
                              ? // A routine's step says which one it is on.
                                ` · ${routineStepPhrase(liveRun.startedBy, routines) ?? 'running'}`
                              : liveRun.phase === 'completed'
                                ? ''
                                : ` · ${liveRun.phase}`
                          }${
                            // This run's own cost. The conversation's total
                            // is NOT here any more -- it is one row inside
                            // the context ring's hover, the surface that was
                            // already conversation-scoped.
                            shownCostTail
                          }`.replace(/^ · /, '')}
                      {/*
                        * The context ring is NOT here.
                        *
                        * It was, from 2026-09-10 -- "a fact about THIS
                        * conversation, and this line is where the
                        * conversation's other facts are" (design agent), after
                        * it had sat on the composer as a bare glyph beside the
                        * swarm mark and read as an artifact rather than a
                        * control. Colin, 2026-09-11: "theres no more
                        * context/usage circle, we can just put it on the left
                        * of the model picker."
                        *
                        * It is there now, and in ONE place: drawing it here as
                        * well would put one fact in two registers. Beside the
                        * route it is no longer loose on the row either -- it
                        * belongs to the control it is about, which is what the
                        * 2026-09-10 objection was really about.
                        */}
                    </div>
                  </div>
                </div>
                <div className="lc-workroom__actions">
                  {/*
                    * One control for the occasional actions, because four
                    * buttons across the top of every conversation is what the
                    * header looked like (Colin, 2026-09-11: "lets clean up
                    * this area with maybe a triple dot dropdown or something").
                    *
                    * Activity stays out of it. It TOGGLES the panel beside the
                    * thread -- it is a view control with a pressed state, and
                    * a thing you turn on and off does not belong behind a menu
                    * that closes when you pick from it. The other three are
                    * each done once and then not again.
                    *
                    * Delete is in here now, and the 2026-09-04 note that kept
                    * it out is answered rather than overruled: its worry was a
                    * menu making the one destructive action harder to find and
                    * three clicks deep. It is NAMED here, which is how a
                    * person finds out an action exists, and the menu asks in
                    * place -- `Delete for good?`, the same second press the
                    * header button had. Nothing about the asking changed.
                    *
                    * Reviewers are listed flat rather than behind a second
                    * menu: `Ask Yurt for a review` is one click instead of
                    * two and says who it goes to before you commit.
                    */}
                  {/* The folder's pull request (0.720): only while its branch has one on GitHub. */}
                  {shownIssue !== undefined && (
                    <button type="button" className="lc-issuechip" title={shownIssue.title} onClick={() => void window.desktop?.openLink(shownIssue.url).catch(() => undefined)}>
                      #{shownIssue.number}
                    </button>
                  )}
                  {workspacePath !== undefined && pickedTeammate?.worktree !== true && liveRun !== undefined && <PullRequestChip running={running} />}
                  {/* Commit (0.680): the folder's own changes. A teammate on its own branch has Review and Land instead. */}
                  {workspacePath !== undefined && pickedTeammate?.worktree !== true && liveRun !== undefined && (
                    <CommitChanges
                      running={running}
                      {...((liveRun.data?.missionId ?? liveRun.restoredMission?.missionId) === undefined ? {} : { of: (liveRun.data?.missionId ?? liveRun.restoredMission?.missionId)! })}
                      asks={[...(liveRun.earlierTurns ?? []).map((turn) => turn.prompt), liveRun.prompt].map((prompt) => splitAttachments(prompt).text).slice(-5)}
                      {...(pickedTeammate === undefined ? {} : { teammate: pickedTeammate.name })}
                      {...(shownIssue === undefined ? {} : { fixes: shownIssue.number })}
                      onCommitted={() => setCommits((count) => count + 1)}
                    />
                  )}
                  {/*
                    * CLAUDE CODE'S ROW OF ICONS (0.797). Colin, 2026-10-19, with Claude Code's header beside it: "lets
                    * lean into something closer to what claude code has, buttons are getting a little too much". The
                    * panels and the folder are icons with their names on hover, in Claude Code's order -- Activity,
                    * Terminal, Changes, Browser, the folder, then the menu -- and Open in terminal, which was here as
                    * a `</>` and first in the menu too, is in the menu only. Commit and the pull request stay words:
                    * they appear only when there is something to do, and say what.
                    */}
                  <button
                    type="button"
                    className={`lc-headicon${inspectorOpen ? ' is-active' : ''}`}
                    aria-label="Activity"
                    aria-pressed={inspectorOpen}
                    title="Activity: what this conversation's runs did"
                    onClick={() => {
                      setReviewingId(undefined)
                      setChangesOpen(false)
                      setInspectorOpen(!inspectorOpen)
                      setTerminalOpen(false)
                      closeBrowserPanel()
                    }}
                  >
                    <Icon name="activity" size={15} />
                  </button>
                  <TerminalToggle open={terminalOpen} refusal={terminalRefusal} onToggle={toggleTerminal} />
                  {/* Review changes (0.439): only a teammate on its own branch has one to review. */}
                  {pickedTeammate?.worktree === true && (
                    <button
                      type="button"
                      className={`lc-headicon${reviewingId === pickedTeammate.teammateId ? ' is-active' : ''}`}
                      aria-label={`Review ${pickedTeammate.name}'s changes`}
                      aria-pressed={reviewingId === pickedTeammate.teammateId}
                      title={`Review changes: what ${pickedTeammate.name} has saved on its own branch, whole or turn by turn`}
                      onClick={() => setReviewingId(reviewingId === pickedTeammate.teammateId ? undefined : pickedTeammate.teammateId)}
                    >
                      <Icon name="diff" size={15} />
                    </button>
                  )}
                  {/* The folder's changes, as Claude Code's Changes panel (0.732); a branch of a teammate's own has Review changes. */}
                  {workspacePath !== undefined && pickedTeammate?.worktree !== true && changesAvailable && (
                    <button
                      type="button"
                      className={`lc-headicon${changesOpen ? ' is-active' : ''}`}
                      aria-label="Changes"
                      aria-pressed={changesOpen}
                      title="Changes: every file this folder's branch changed, against where it left its base"
                      onClick={() => {
                        setReviewingId(undefined)
                        setInspectorOpen(false)
                        setChangesOpen(!changesOpen)
                        setTerminalOpen(false)
                        closeBrowserPanel()
                      }}
                    >
                      <Icon name="diff" size={15} />
                    </button>
                  )}
                  <BrowserToggle open={browserOpen} refusal={browserRefusal} onToggle={toggleBrowser} />
                  {workspacePath !== undefined && (
                    <button
                      type="button"
                      className="lc-headicon"
                      aria-label={`Open ${workspaceName.length === 0 ? 'the folder' : workspaceName} in the file manager`}
                      title={folderProblem ?? `Open the folder: ${workspacePath}`}
                      onClick={() => {
                        void window.desktop?.openWorkspaceFolder().then((answer) => setFolderProblem(answer.ok ? undefined : answer.message)).catch(() => undefined)
                      }}
                    >
                      <Icon name="folder" size={15} />
                    </button>
                  )}
                  {headerActions.length > 0 && (
                    <button
                      type="button"
                      className="lc-headicon"
                      aria-label="More actions"
                      aria-haspopup="menu"
                      title="More actions"
                      onClick={(event) => {
                        const at = event.currentTarget.getBoundingClientRect()
                        setRowMenu({
                          // Right-aligned under the button: the menu is wider
                          // than the control and the header sits at the window
                          // edge, so hanging it from the left corner pushed it
                          // off screen.
                          x: Math.round(Math.max(8, at.right - 220)),
                          y: Math.round(at.bottom + 4),
                          title: 'This conversation',
                          items: headerActions
                        })
                      }}
                    >
                      <Icon name="dots" size={15} />
                    </button>
                  )}
                </div>
              </header>
              {/*
                * Only while something is actually running. It carries a live
                * budget and a Stop control, and neither means anything once
                * the exchange is over -- after which it is a permanent header
                * on a conversation that has finished (Colin, 2026-09-06:
                * "dont have that exchange thing always up there, we want less
                * clutter"). What it said is still in the thread and the record.
                */}
              {exchange !== undefined && (
                <ExchangeStrip
                  exchange={exchange}
                  cap={relayHopCap}
                  runtimeNameOf={runtimeNameOf}
                  stopping={stoppingExchange}
                  onStop={() => stopExchange(exchange.liveRunIds)}
                />
              )}
              {(() => {
                // A conversation that began as a comparison says so, and opens it (0.441).
                const shownId = liveRun.data?.missionId
                const row = shownId === undefined ? undefined : sidebarMissions.find((entry) => (entry.memberIds ?? [entry.missionId]).includes(shownId))
                // Or found from the answer itself, however it was opened (0.539): Sol and Cursor opened
                // a column from Conversations, before Keep, and found no way back to its comparison.
                const compare = (row === undefined ? undefined : comparisonOf(row.memberIds ?? [row.missionId], compares))
                  ?? (shownId === undefined ? undefined : compares.find((one) => one.slots.some((column) => column.missionIds.includes(shownId))))
                if (compare === undefined) return null
                if (compare.kept === undefined) {
                  return (
                    <div className="lc-compared">
                      <span>This answer is one column of a comparison you have not kept an answer from yet.</span>
                      <button type="button" className="lc-compared__open" onClick={() => setComparingId(compare.compareId)}>
                        Open the comparison
                      </button>
                    </div>
                  )
                }
                const others = compare.slots.filter((column) => column.slot !== compare.kept!.slot).map((column) => column.route.label ?? column.route.model)
                // A tune (0.809) is one model three ways: "Compared with Deepseek and Deepseek" named the model twice
                // and never the setup that won.
                const keptRoute = compare.slots.find((column) => column.slot === compare.kept!.slot)?.route
                const keptSetup = keptRoute === undefined ? undefined : setupName(keptRoute)
                const tunedOn = keptRoute?.label?.split(' · ')[0] ?? keptRoute?.model
                return (
                  <div className="lc-compared">
                    <span>
                      {keptSetup !== undefined
                        ? `You tried ${String(compare.slots.length)} sets of instructions on ${tunedOn ?? 'this model'} and kept ${keptSetup}.`
                        : `Compared with ${others.join(' and ')}.`}
                      {compare.kept.brought !== undefined &&
                        (compare.kept.brought.length === 0
                          ? ' It changed nothing, so nothing came into your folder.'
                          : ` Its changes came into your folder: ${compare.kept.brought.length === 1 ? compare.kept.brought[0]! : `${String(compare.kept.brought.length)} files`}.`)}
                    </span>
                    <button type="button" className="lc-compared__open" onClick={() => setComparingId(compare.compareId)}>
                      Open the comparison
                    </button>
                  </div>
                )
              })()}
              {(() => {
                const compare = comparisonOf([...turnsOfConversation(liveRun.data?.missionId, sidebarMissions)], compares)
                return compare === undefined ? null : <KeptInstructions key={compare.compareId} compare={compare} teammate={teammates.find((mate) => mate.teammateId === compare.teammateId)} onUse={useTunedInstructions} />
              })()}
              <DiffNotesContext.Provider value={diffNotesPlace}>
              <Thread
                findRequest={findRequest}
                backgroundTurns={backgroundTurnIds}
                onOpenFile={openFileInViewer}
                // A status chip changed in a teammate's tracker writes its ask into the box (0.733).
                onDraft={(text) => setHandBack({ text, attachments: [] })}
                prompt={liveRun.prompt}
                startedBy={liveRun.startedBy}
                {...(liveRun.data?.mode === undefined ? {} : { runMode: liveRun.data.mode })}
                planMode={liveRun.plan === true}
                {...startingLabelOf(liveRun)}
                onOpenPeerRun={(messageId) => {
                  // Only where the record shows the message actually reached
                  // a run. Nothing received it yet is a real state -- it
                  // waits for that teammate's next run -- and a control that
                  // led nowhere would say otherwise.
                  const reached = peerRunFor(messageId, history)
                  return reached === undefined ? undefined : () => openMission(reached.missionId)
                }}
                onOpenSenderRun={(missionId) => () => openMission(missionId)}
                earlierTurns={liveRun.earlierTurns ?? []}
                calls={threadCalls}
                cloud={threadCloud}
                {...inTerminalOf(liveRun)}
                {...(() => {
                  // Where this conversation's group began briefing it. The
                  // row knows every id the conversation has worn; the
                  // membership is read against all of them, the way the
                  // sidebar reads it; the turns' start times place the line.
                  const shownId = liveRun.data?.missionId ?? liveRun.restoredMission?.missionId
                  const row =
                    shownId === undefined
                      ? undefined
                      : sidebarMissions.find((entry) => (entry.memberIds ?? [entry.missionId]).includes(shownId))
                  const membership = row === undefined ? undefined : heldFor(row, groupMembers)
                  const group = membership === undefined ? undefined : groups.find((entry) => entry.groupId === membership.groupId)
                  const starts = [
                    ...(liveRun.earlierTurns ?? []).map((turn) => historyByIdRef.current.get(turn.missionId)?.createdAt),
                    liveRun.startedAtIso ?? liveRun.restoredMission?.createdAt
                  ]
                  const boundary = groupBoundary(starts, membership, group)
                  // And where earlier groups' words stopped: every id the
                  // conversation has worn, the same way membership is read.
                  const left = row === undefined ? undefined : heldFor(row, groupLeft)
                  return {
                    ...(boundary === undefined ? {} : { groupBoundary: boundary }),
                    // And where they STARTED, for memberships that have ended:
                    // the turns those words briefed still say so.
                    pastBoundaries: groupJoins(starts, left ?? []),
                    groupLeavings: groupLeavings(starts, left ?? [])
                  }
                })()}
                coldStart={liveRun.coldStart ?? false}
                rewound={liveRun.rewound === true}
                {...(liveRun.putBack === undefined ? {} : { putBack: liveRun.putBack })}
                {...(liveRun.versions === undefined ? {} : { versions: liveRun.versions })}
                onOpenVersion={openMission}
                {...(running || liveRun.data === undefined || comparing !== undefined ? {} : { onEditMessage: editEarlierMessage })}
                workspacePath={workspacePath}
                {...(workspaceId === undefined ? {} : { workspaceId })}
                wasPlan={liveRun.plan === true}
                onRunWithEdits={
                  // Offered only where it is genuinely the next thing a
                  // person wants: a finished READ-ONLY run whose reply
                  // carries code the runtime was not allowed to apply. Not
                  // an error -- the run did what its mode permits -- just the
                  // one click that would otherwise be a mode change and a
                  // retyped prompt. Thread decides whether code is present.
                  !running
                  && liveRun.data?.sandbox === 'read-only'
                  && liveRun.prompt.trim().length > 0
                  && modeRunsOn('accept-edits', liveRun.data.runtime, build?.platform)
                    ? () => {
                        // An ordinary mode switch now, named on the band
                        // above the button before it is pressed.
                        setMode('accept-edits')
                        void startMission(liveRun.prompt, 'accept-edits')
                      }
                    : undefined
                }
                {...runAgainOffer(liveRun, running, () => void startMission(liveRun.prompt))}
                resumeRefusal={resumeRefusal !== undefined && resumeRefusal.missionId === liveRun.restoredMission?.missionId ? resumeRefusal.message : undefined}
                onResume={
                  // Offered only for a mission that is not running and whose
                  // record the app will vouch for -- ResumeCard decides that
                  // from the checkpoint, and the host checks it again rather
                  // than taking the renderer's word.
                  !running && !handingOff && liveRun.restoredMission !== undefined
                    ? (epoch) => {
                        void resumeMission(liveRun.restoredMission!.missionId, epoch)
                      }
                    : undefined
                }
                onAnswer={
                  // Answering IS the next turn of the conversation, so it goes
                  // through the same path a typed reply does -- same continuity,
                  // same route, same record. A run still going has not asked
                  // anything yet, and one with no prompt cannot be continued.
                  !running && liveRun.prompt.trim().length > 0
                    ? (option) => {
                        void startMission(decisionReply(option))
                      }
                    : undefined
                }
                {...(liveRun.data?.sandbox === undefined ? {} : { sandbox: liveRun.data.sandbox })}
                events={liveRun.events}
                eventsTruncated={liveRun.eventsTruncated === true}
                running={running}
                {...(busyOffer === undefined ? {} : { busyModel: busyOffer })}
                /* The same press as the chat box's Stop, from a live line gone quiet (0.758). */
                onStopRun={cancelMission}
                {...(limitOffer === undefined ? {} : { limitModel: limitOffer })}
                {...(continueOffer === undefined || running ? {} : { onContinueAfterBusy: continueOffer })}
                {...(carryOnShown === undefined || running ? {} : { carryOn: carryOnShown })}
                {...(signInOffer === undefined ? {} : { signInRuntime: signInOffer })}
                onSendAgain={
                  // R29: a run stopped before it used any tool, most often
                  // by the second click of a double click on Send. Thread
                  // offers it only on that card; sending again cannot repeat
                  // anything, because nothing ran.
                  !running && liveRun.phase === 'cancelled' && liveRun.prompt.trim().length > 0
                    ? () => void startMission(liveRun.prompt)
                    : undefined
                }
                restoredMission={liveRun.restored === true ? liveRun.restoredMission : undefined}
                {...(liveRun.data?.missionId === undefined ? {} : { shownMissionId: liveRun.data.missionId })}
                error={liveRun.error}
                errorIsPersistence={liveRun.errorIsPersistence === true}
                {...(ledgerPath === undefined ? {} : { ledgerPath })}
                approvals={shownApprovals}
                onDecide={decideApproval}
                onAnswerQuestion={answerQuestion}
                decidingIds={decidingIds}
                cancelled={liveRun.phase === 'cancelled'}
                stoppedByPerson={liveRun.stopPressed === true}
                handoff={liveRun.handoff}
                {...(liveRun.switchedFrom === undefined ? {} : { switchedFrom: liveRun.switchedFrom })}
                {...((() => {
                  const check = (liveRun.data?.missionId === undefined ? undefined : editChecks.get(liveRun.data.missionId)) ?? liveRun.editCheck
                  return check === undefined ? {} : { editCheck: check }
                })())}
                {...(running ? {} : { onSendEditCheck: (text: string) => { void startMission(text) } })}
                peers={{
                  self: missionOwner,
                  teammates,
                  messages: liveRun.peerMessages ?? [],
                  notices: liveRun.peerNotices ?? [],
                  /*
                   * What this CONVERSATION taught the team, read from the
                   * memory list rather than from a notice that would be gone
                   * once the thread is drawn from the record.
                   *
                   * Every turn of it, not the turn on screen. A reply is its
                   * own mission, so matching the shown missionId lost every
                   * memory learned earlier the moment a later turn began --
                   * Colin, 2026-09-10: "after an agent 'remembers something'
                   * it disappears in chat". The header's Delete had to learn
                   * the same lesson about the same shape.
                   */
                  memories: memoriesOfConversation(
                    memories,
                    turnsOfConversation(liveRun.data?.missionId, sidebarMissions)
                  )
                }}
                startedAt={
                  liveRun.restoredMission === undefined
                    ? undefined
                    : startedLabel(liveRun.restoredMission.createdAt)
                }
                {...(() => {
                  const iso = liveRun.startedAtIso ?? liveRun.restoredMission?.createdAt
                  return iso === undefined ? {} : { startedAtIso: iso }
                })()}
              />
              </DiffNotesContext.Provider>
            </>
          )}
          {/*
            * A refused delete, wherever it was asked for. This used to render
            * inside the shown conversation's header -- so a Delete refused
            * from the SIDEBAR, on a row you were not looking at, reported
            * nothing anywhere and read as the menu doing nothing at all
            * (Colin, 2026-09-05, second report of "delete not working").
            */}
          {deleteError !== undefined && (
            <div className="lc-diagnostic lc-tone-red" role="alert">
              <Icon name="shield" size={12} />
              <span>{deleteError}</span>
            </div>
          )}
          {terminalError !== undefined && (
            <div className="lc-diagnostic lc-tone-amber" role="alert">
              <Icon name="code" size={12} />
              <span>{terminalError}</span>
            </div>
          )}
          {rowNotice !== undefined && (
            <div className="lc-diagnostic lc-tone-green" role="status">
              <Icon name="check" size={12} />
              <span>{rowNotice}</span>
            </div>
          )}
          {talkNotice !== undefined && (
            <div className="lc-diagnostic lc-tone-amber" role="alert" data-talk-notice="">
              <Icon name="waveform" size={12} />
              <span>{talkNotice}</span>
            </div>
          )}
          {lastRestore !== undefined && (
            <div className={`lc-diagnostic ${lastRestore.ok ? 'lc-tone-green' : 'lc-tone-amber'} lc-diagnostic--row lc-restored`} role="status">
              <Icon name={lastRestore.ok ? 'check' : 'shield'} size={12} />
              <span>{restoreNoticeLine(lastRestore)}</span>
              <button type="button" className="lc-button" onClick={() => setLastRestore(undefined)}>
                OK
              </button>
            </div>
          )}
          {workspaceNotice !== undefined && (
            <div className="lc-diagnostic lc-tone-amber lc-diagnostic--row" role="alert">
              <Icon name="shield" size={12} />
              <span>{workspaceNotice}</span>
              {workspacePath === undefined && (
                <button type="button" className="lc-button" onClick={chooseWorkspace}>
                  Choose folder
                </button>
              )}
            </div>
          )}
          {/* What changed lives in Settings > What's new now, and a big build
              gets the splash (see `splashOpen`); the banner that sat here is gone. */}
          {/*
            * Not while this conversation's run is going: the outside recheck
            * of 0.303 found the banner "understandable, but competed for
            * attention during a live mission" -- and installing is refused
            * mid-run anyway. It is there again the moment the run ends.
            */}
          {screen === 'workroom' && !running && <UpdateBanner update={update} onInstall={installUpdate} />}
          {screen === 'workroom' && !running && (
            <MacUpdateBanner release={macRelease} onDownload={(url) => void window.desktop?.openLink(url).catch(() => undefined)} />
          )}
          {screen === 'workroom' && pullRequestNewsShown !== undefined && (
            <PullRequestNewsBanner
              news={pullRequestNewsShown}
              teammateName={pickedTeammate?.name}
              onAsk={(text) => {
                setHandBack({ text, attachments: [] })
                setPullRequestNewsShown(undefined)
              }}
              onOpen={(url) => void window.desktop?.openLink(url).catch(() => undefined)}
              onDismiss={() => setPullRequestNewsShown(undefined)}
            />
          )}
          {screen === 'workroom' && commandEntries.some((entry) => entry.key === composerDraftKey && !entry.carried) && (
            <div className="lc-commandlog">
              {commandEntries.filter((entry) => entry.key === composerDraftKey && !entry.carried).map((entry) => (
                <CommandEntry key={entry.id} command={entry} onStop={(id) => { void window.desktop?.ownCommands?.stop(id) }} />
              ))}
            </div>
          )}
          {screen === 'workroom' && (
          <Composer
            draftKey={composerDraftKey}
            {...(shownData?.missionId === undefined ? {} : { referenceMissionId: shownData.missionId })}
            {...(referenceCwd === undefined ? {} : { referenceCwd })}
            referencesOnCloud={cloudOn || computerOn}
            ownCommands={ownCommandMenu}
            onChooseOwnCommand={chooseOwnCommand}
            onRunCommand={runOwnCommand}
            {...(commandAsk === undefined ? {} : { commandAsk: { lines: commandAsk.lines, onAllow: allowCommandAsk, onNotNow: () => setCommandAsk(undefined) } })}
            issues={githubReady}
            onPickIssue={openIssueList}
            canTune={tuneRefusal(missionOwner, liveRun?.data?.missionId, liveRun !== undefined && isTerminal(liveRun.phase) && comparing === undefined) === undefined}
            onTune={() => { openTune() }}
            {...(issuePicker === undefined ? {} : { issueList: { issues: issuePicker, onChoose: chooseIssue, onClose: () => setIssuePicker(undefined), ...(issueEmpty === undefined ? {} : { empty: issueEmpty }) } })}
            // Talk in a teammate's conversation is a call with them, into it (0.746).
            // With the call drawn in this conversation, a connector's question is a card in it (0.749).
            {...(talkTo === undefined ? {} : { talkTo: { ...talkTo, asksInThread: threadCalls.some((call) => 'live' in call && call.live) } })}
            metal={metal}
            metalStrength={metalStrength}
            metalMotion={metalMotion}
            metalBend={metalBend}
            // So the mode menu can say where connectors live when this route
            // is not the one they are signed in on. See `connectorsNote`.
            hasConnectors={connectorList !== undefined && connectorList.length > 0}
            // A teammate on Claude Code with none ticked runs without the person's connectors (0.798).
            {...(composerRoute.runtime === 'claude' && pickedTeammate !== undefined && (pickedTeammate.connectors ?? []).length === 0 && connectorList !== undefined && connectorList.length > 0
              ? { connectorsOff: { teammate: pickedTeammate.name, names: connectorList.map((connector) => connector.name) } }
              : {})}
            // Said before the send: a reply on another runtime continues
            // from the stopped run's checkpoint, not from its memory.
            continuationNote={continuationNote ?? newConversationNote}
            {...(continuation === undefined ? {} : { continuation })}
            workspaceName={workspaceName.length === 0 ? undefined : workspaceName}
            workspacePath={workspacePath}
            workspaceMade={workspaceMade}
            {...(workspaceCaution === undefined ? {} : { workspaceCaution })}
            {...(folderQuestion === undefined || folderQuestion.key !== shownKey ? {} : { folderQuestion: { name: workspaceName, why: folderQuestion.why }, onConfirmFolder: confirmFolder })}
            onChooseFolder={chooseWorkspace}
            folders={folders.filter((folder) => folder.installFolder !== true)}
            {...(workspaceId === undefined ? {} : { currentFolderId: workspaceId })}
            // Another folder from the chip starts a new conversation there, as in Claude Code (0.458).
            onSwitchFolder={(id) => {
              setShownKey(undefined)
              switchToFolder(id)
            }}
            runtimes={runtimes}
            runtimeCommands={runtimeCommands}
            runtimesGaveUp={runtimeState.phase === 'ready' && runtimeState.gaveUp === true}
            limitedRuntimes={limitedRuntimes}
              usageWindows={usageWindows}
            discoveryPhase={runtimeState.phase}
            {...(shownContext === undefined ? {} : { context: shownContext })}
            {...(shownConversationCost === undefined ? {} : { conversationCost: shownConversationCost })}
            running={running}
            cancelling={liveRun?.phase === 'cancelling'}
            activeRoute={liveRun?.data}
            // What the composer SHOWS has to be what will actually run.
            // A teammate can carry a stored mode their route cannot honour
            // (Claude Code kept on Accept edits from before), and showing it
            // would promise an edit the host is about to refuse.
            mode={
              modeRunsOn(mode, composerRoute.runtime, build?.platform)
                ? mode
                : modesFor(composerRoute.runtime, build?.platform)[0] ?? mode
            }
            onModeChange={setMode}
            autoMode={autoMode}
            onEnableAutoMode={() => {
              // Picking Auto in the composer IS the person switching it on.
              // Written through the host like any other settings change, so
              // the next run -- and a relay hop or a routine step -- reads the
              // same answer the composer just showed.
              setAutoMode(true)
              void window.desktop
                ?.writeWorkspaceSettings({ swarm, relay, relayHopCap, interrupt, memoryMode, autoMode: true, askConnectors, keepATodoList, replySize, layout, tube })
                .then((settings) => setAutoMode(settings.autoMode === true))
                .catch(() => setAutoMode(false))
            }}
            route={composerRoute}
            onRouteChange={changeRoute}
            {...(handBack === undefined ? {} : { handBack })}
            {...(quoteIn === undefined ? {} : { quoteIn })}
            pagePoints={pagePoints}
            onPagePointNote={(id, note) => setPagePoints((current) => noteOnPagePoint(current, id, note))}
            onPagePointRemove={(id) => setPagePoints((current) => withoutPagePoint(current, id))}
            onPagePointsSent={() => setPagePoints([])}
            models={models}
            diffNotes={shownNotes}
            onClearDiffNotes={() => changeShownNotes(() => [])}
            resolvedModels={resolvedModels}
            recentRoutes={recentRoutes}
            platform={build?.platform}
            effort={effort}
            onEffortChange={setEffort}
            swarm={swarm}
            onSwarmChange={(next) => {
              // Optimistic, then reconciled with what the store actually
              // saved -- a rejected write must not leave the chip claiming a
              // setting that is not on disk.
              setSwarm(next)
              if (next) setSwarmCalls((count) => count + 1)
              void window.desktop
                ?.writeWorkspaceSettings({ swarm: next, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube })
                .then((settings) => setSwarm(settings.swarm === true))
                .catch(() => setSwarm(!next))
            }}
            error={
              /*
               * Only when something is standing: a runtime that is here and
               * not usable (signed out, not answering). On a launch with
               * NOTHING installed the card is cut -- design agent,
               * 2026-09-18: "Red in this app means a run stopped. Nothing
               * stopped here -- nothing has started, and never could have."
               * The disabled composer under it demonstrates the same fact
               * and its placeholder says what would change it; an assertion
               * four lines above a demonstration is the assertion losing.
               */
              /*
               * And not while the rows still say CHECKING. Colin, 2026-09-19,
               * with a frame of five rows checking, the no-Node sentence and
               * this card all at once: "this cant be ideal." A probe that
               * has not answered is not a thing standing either; it is a
               * wait. Standing means a runtime that is here and has SAID it
               * cannot run -- signed out, or never answering after the
               * re-checks ran out.
               */
              noRuntimeReady &&
              runtimeState.phase === 'ready' &&
              /*
               * Not after discovery gives up, either. 0.185.0 counted "asked
               * four times, no answer" as something standing, and Grok's
               * pass 14 drew what that meant: five rows already reading
               * NOT ANSWERING with Check again beside each, a composer that
               * already cannot send and already says why, and THEN this
               * card above it. The rows are the repair; the card was a
               * third telling. It stays for a sign-in, which no row can
               * perform for you.
               */
              runtimeState.runtimes.some(
                (runtime) => runtime.installed && (runtime.status === 'auth-required' || runtime.auth === 'unauthenticated')
              )
                ? 'No AI agent can start yet. Locust works through the AI agents installed on this machine — Settings shows what to install, and OpenCode needs no account.'
                : undefined
            }
            onStart={sendOrCompare}
            // Compare (0.441): the picker's switch, and a comparison on screen.
            {...(comparePicking === undefined ? {} : { compare: comparePicking })}
            {...(composerRoute.runtime === 'claude' && comparing === undefined
              ? {
                  background: {
                    on: backgroundOn,
                    onMode: (on: boolean) => {
                      setBackgroundOn(on)
                      if (on) {
                        setCloudOn(false)
                        setCloudPanel(false)
                        setBackgroundPanel(true)
                      }
                    }
                  }
                }
              : {})}
            cloud={{
              on: cloudOn,
              onMode: (on) => {
                setCloudOn(on)
                // The person's own cloud computer (0.761): on at once; the first time for a repository, the dialog.
                if (on && composerRoute.runtime === 'opencode') {
                  void checkCloudComputer()
                  return
                }
                if (on) {
                  setCloudPanel(true)
                  void window.desktop?.cloudWhere().then(setCloudWhere).catch(() => undefined)
                  void window.desktop?.listClaudeCloud().then(setClaudeCloud).catch(() => undefined)
                }
              },
              where: composerRoute.runtime === 'claude' ? 'claude' : composerRoute.runtime === 'opencode' ? 'computer' : 'codex',
              // Claude Code and Codex on the person's own cloud computer too (0.768), beside their makers' clouds.
              ...(composerRoute.runtime === 'claude' || composerRoute.runtime === 'codex'
                ? {
                    computer: {
                      on: computerOn,
                      agentName: CLOUD_AGENT_NAMES[composerRoute.runtime],
                      onMode: (on: boolean) => {
                        setComputerOn(on)
                        if (on) void checkCloudComputer(composerRoute.runtime === 'claude' ? 'claude' : 'codex')
                      }
                    }
                  }
                : {}),
              environment: {
                value: cloudEnvironment.folder === workspacePath ? cloudEnvironment.value : '',
                onChange: (value) => setCloudEnvironment({ folder: workspacePath ?? '', value, edited: true })
              },
              ...(composerRoute.runtime === 'claude' || composerRoute.runtime === 'opencode'
                ? {}
                : composerRoute.runtime !== 'codex'
                ? { refusal: 'Cloud runs on Codex, Claude or OpenCode: pick one of their models first.' }
                : cloudWhere?.codexReady === false
                  ? { refusal: 'Codex CLI is not installed or not signed in here.' }
                  // Not on GitHub is not refused here (0.504): the panel says so, with the folders that are.
                  : {})
            }}
            pickerRequest={pickerRequest}
            {...(composerFill === undefined ? {} : { fill: composerFill })}
            {...(rewinding === undefined || rewinding.key !== shownKey
              ? {}
              : {
                  editingEarlier: {
                    onCancel: () => {
                      setRewinding(undefined)
                      setComposerFill((current) => ({ text: '', seq: (current?.seq ?? 0) + 1 }))
                    },
                    ...(rewinding.files.length === 0
                      ? {}
                      : rewinding.files.some((file) => file.cannot !== undefined)
                        // Known already: some cannot go back exactly, so none are offered (0.512).
                        ? { filesHeld: { total: rewinding.files.length, cannot: rewinding.files.filter((file) => file.cannot !== undefined).length } }
                        : {
                          files: {
                            count: rewinding.files.length,
                            on: rewinding.putBack,
                            onToggle: () => setRewinding((current) => (current === undefined ? current : { ...current, putBack: !current.putBack }))
                          }
                        })
                  }
                })}
            {...(comparing === undefined || comparing.kept !== undefined
              ? {}
              : { asking: { label: comparing.slots.map((column) => (comparing.blind === true ? blindName(column.slot, hasColumnInstructions(comparing)) : column.route.label ?? column.route.model)).join(' vs '), columns: comparing.slots.filter((column) => column.missionIds.length > 0).length, ...(comparing.changes === true ? { changes: true } : {}), ...(comparing.blind === true ? { blind: true } : {}) } })}
            // Tag a teammate from any conversation (0.438).
            team={teammates}
            {...(pickedTeammate === undefined ? {} : { currentTeammateId: pickedTeammate.teammateId })}
            onTag={tagTeammates}
            onCancel={cancelMission}
            onOpenRoutePicker={() => {
              // Opening the picker is the moment the list matters most, and
              // the cheapest moment to be sure it is current.
              readModels()
            }}
            onHandOff={handOffMission}
            handingOff={handingOff}
            teammateName={pickedTeammate?.name}
            // Nobody's conversation names the agent it runs on: "it goes to This teammate" read wrong (Colin, 0.570).
            busyWith={busyRun === undefined ? undefined : (pickedTeammate?.name ?? (busyRun.runtime === undefined ? 'this teammate' : runtimeNameOf(busyRun.runtime)))}
            // What is waiting, as the person will see it go: folded, so the
            // strip shows the one instruction that will actually be sent
            // rather than the pieces it was typed in.
            queued={combineQueued(waitingHere)[0]?.text}
            queuedCount={waitingHere.length}
            queuedAttachments={combineQueued(waitingHere)[0]?.attachments}
            queuedNote={queueError ?? queuedNote}
            queueError={queueError}
            queuedElsewhere={queueKey !== shownKey}
            onQueue={async (text, attachments, now) => {
              // Into the conversation the box shows the queue of (queueKey):
              // the live run on screen, else the addressed teammate's busy run.
              const key = queueKey
              if (key !== undefined) {
                const queuedIssue = pendingIssueRef.current !== undefined && text !== '/compact' && text.includes(`GitHub issue #${String(pendingIssueRef.current.number)}`) ? pendingIssueRef.current : undefined
                if (queuedIssue !== undefined) pendingIssueRef.current = undefined
                const saved = await setQueued((rows) => [
                  ...rows,
                  { id: `q_${String(rows.length)}_${key}`, key, text, origin: 'person' as const, ...(attachments.length === 0 ? {} : { attachments }), ...(waitForKey === undefined ? {} : { waitFor: waitForKey }), ...(queuedIssue === undefined ? {} : { issue: queuedIssue }) }
                ])
                // Ctrl+Enter: stop the run on screen and send this at once.
                if (saved && now === true && key === liveOnScreen) {
                  setSendAfterStop(key)
                  cancelMission()
                }
                return saved
              }
              return false
            }}
            {...(queueKey !== undefined && queueKey === liveOnScreen
              ? {
                  onSendQueuedNow: () => {
                    setSendAfterStop(queueKey)
                    cancelMission()
                  }
                }
              : {})}
            onUnqueue={() => setQueued((rows) => withoutQueueOf(rows, queueKey))}
            onSendQueued={() => {
              if (queueKey === undefined) return
              void sendQueued(queueKey, (going) => startMission(withAttachments(going.text, going.attachments ?? []), undefined, { requeue: going }))
            }}
          />
          )}
        </main>
        {/*
          * Only beside the conversation it inspects. It used to stay open
          * across All missions, Settings and Team, where it inspected
          * nothing on screen and took 344px from screens that had not been
          * drawn for the squeeze: at 1120 the Settings pane and the mission
          * rows grew sideways scrollbars (crowded-window drive, 2026-09-19).
          * It comes back with the workroom; the toggle keeps its state.
          */}
        {/*
          * ONE REGION, TWO OCCUPANTS, and the file wins.
          *
          * A file is opened by pressing a specific file; the inspector is a
          * toggle. The more specific intent is the one to honour, and the
          * inspector is one press away again the moment the file is closed.
          */}
        {terminalMounted && <Suspense fallback={null}><TerminalPanel visible={terminalOpen && screen === 'workroom'}
          context={{ ...(terminalMissionId === undefined ? {} : { missionId: terminalMissionId }), ...(pickedTeammate === undefined ? {} : { teammateId: pickedTeammate.teammateId }) }}
          onClose={() => setTerminalOpen(false)} /></Suspense>}
        {browserMounted && <Suspense fallback={null}><BrowserPanel visible={browserOpen && screen === 'workroom'}
          runtime={liveRun?.data?.runtime ?? composerRoute.runtime}
          {...(browserRefusal === undefined ? {} : { pointingOff: browserRefusal })}
          onPointed={async (pick) => {
            if (pagePoints.length >= MAX_PAGE_POINTS) {
              return 'Eight parts are already in the box. Remove one to point at another. The page was not added.'
            }
            const bridge = window.desktop
            const named = `page-part-${String(Date.now())}.png`
            const answer = pick.png === undefined || bridge === undefined ? undefined : await bridge.attachPasted(named, pick.png).catch(() => undefined)
            const attachment = answer?.ok === true ? answer.paths[0] : undefined
            const chip: PagePointChip = {
              id: `page-point-${String(Date.now())}-${Math.random().toString(36).slice(2, 8)}`,
              point: pick.point,
              note: '',
              ...(attachment === undefined ? {} : { attachment })
            }
            setPagePoints((current) => addPagePoint(current, chip).points)
          }}
          onClose={closeBrowserPanel} /></Suspense>}
        {!terminalOpen && !browserOpen && viewerRefusal !== undefined && screen === 'workroom' && viewingFile === undefined && (
          <aside className="lc-viewer lc-viewer--refused" role="status">
            <div className="lc-viewer__head">
              <span className="lc-viewer__name">{refusedName === undefined || refusedName.length === 0 ? 'Could not open that file' : `Could not open ${refusedName}`}</span>
              <span className="lc-viewer__spacer" />
              <button type="button" className="lc-viewer__close" aria-label="Dismiss" onClick={() => setViewerRefusal(undefined)}>
                <Icon name="close" size={13} />
              </button>
            </div>
            <p className="lc-viewer__refusal">{viewerRefusal}</p>
          </aside>
        )}
        {viewingFile !== undefined && screen === 'workroom' ? (
          <FileViewer
            // Keyed by the file, so a new file opens on itself and not on the
            // version the last one was showing (L21).
            key={viewingFile.path}
            path={viewingFile.path}
            text={viewingFile.text}
            mode={viewingFile.mode}
            {...(viewingFile.workbook === undefined ? {} : { workbook: viewingFile.workbook })}
            {...(viewingFile.document === undefined ? {} : { document: viewingFile.document })}
            {...(viewingFile.pageUrl === undefined ? {} : { pageUrl: viewingFile.pageUrl })}
            {...(viewingFile.pdf === undefined ? {} : { pdf: viewingFile.pdf })}
            onPointAt={(pick) => {
              // The picture goes the way a pasted one does (attachPasted), then
              // the quote and the picture land in the chat box together.
              const bridge = window.desktop
              void (async () => {
                const named = `page-part-${String(Date.now())}.png`
                const answer = pick.png === undefined || bridge === undefined ? undefined : await bridge.attachPasted(named, pick.png).catch(() => undefined)
                setQuoteIn({ quote: pick.quote, ...(answer?.ok === true ? { attachments: answer.paths } : {}) })
              })()
            }}
            turns={viewingFileTurns}
            onClose={() => setViewingFile(undefined)}
            onReveal={() => {
              const bridge = window.desktop
              if (bridge === undefined || workspacePath === undefined) return
              void bridge.revealFile(viewingFile.path).catch(() => undefined)
            }}
            onSave={() => {
              const bridge = window.desktop
              if (bridge === undefined || workspacePath === undefined) return
              void bridge.saveCopy(viewingFile.path).catch(() => undefined)
            }}
          />
        ) : backgroundPanel && screen === 'workroom' ? (
          <BackgroundRuns
            runs={backgroundRuns}
            {...(backgroundNote === undefined ? {} : { note: backgroundNote })}
            onOpen={(id) => void window.desktop?.openBackground(id).then((answer) => { if (!answer.ok && answer.message !== undefined) setBackgroundNote({ text: answer.message }) })}
            onStop={(id) => void window.desktop?.stopBackground(id)}
            onShow={(missionId) => openMission(missionId)}
            onDismiss={(id) => void window.desktop?.dismissBackground(id).then(() => setBackgroundRuns((current) => current.filter((run) => run.id !== id)))}
            onSetUp={() => void window.desktop?.setUpBackground().then((answer) => setBackgroundNote(answer.ok ? { text: 'Claude Code is open in a terminal. Answer its question about this folder, then send again.' } : { text: answer.message ?? 'Claude Code could not be opened. Nothing was sent; open it in this folder yourself, answer its question, then send again.' }))}
            onClose={() => setBackgroundPanel(false)}
          />
        ) : cloudPanel && screen === 'workroom' ? (
          <CloudTasks
            tasks={cloudTasks}
            where={cloudWhere}
            notes={cloudNotes}
            problem={cloudProblem}
            applying={cloudApplying}
            onShowChange={async (taskId) => (await window.desktop?.cloudTaskDiff(taskId).catch(() => undefined)) ?? undefined}
            onApply={(taskId) => {
              setCloudApplying(taskId)
              void window.desktop?.applyCloudTask(taskId).then((answer) => {
                if (answer.ok) {
                  setCloudTasks((current) => current.map((task) => (task.taskId === taskId ? answer.task : task)))
                  setCloudProblem(undefined)
                } else {
                  setCloudProblem(answer.message)
                }
              }).catch(() => setCloudProblem('The change could not be applied. Nothing in the folder was changed by Locust.')).finally(() => setCloudApplying(undefined))
            }}
            onOpen={(url) => void window.desktop?.openLink(url).catch(() => undefined)}
            onClose={() => setCloudPanel(false)}
            folders={cloudFolders}
            onOpenFolder={(id) => switchToFolder(id)}
            onChooseFolder={() => void chooseWorkspace()}
            claude={{
              picked: composerRoute.runtime === 'claude',
              sessions: claudeCloud,
              note: claudeCloudNote,
              onHome: (id) => {
                void window.desktop?.bringClaudeCloudHome(id).then((answer) => {
                  const known = claudeCloud.find((one) => one.id === id)?.sessionId !== undefined
                  setClaudeCloudNote(answer.ok
                    ? known
                      ? 'Claude Code opened in this folder and is bringing this session’s work into it.'
                      : 'Claude Code opened in this folder with its list of cloud sessions. Pick this one, and its work comes into this folder.'
                    : answer.message)
                }).catch(() => setClaudeCloudNote('Claude Code could not be opened. Run claude --teleport in a terminal in this folder to bring it home.'))
              },
              onForget: (id) => {
                setClaudeCloud((current) => current.filter((one) => one.id !== id))
                void window.desktop?.forgetClaudeCloud(id).catch(() => undefined)
              },
              onOpenWeb: (url) => void window.desktop?.openLink(url ?? 'https://claude.ai/code').catch(() => undefined),
              onContinue: async (id) => {
                const answer = await window.desktop?.continueClaudeCloudInTerminal(id).catch(() => undefined)
                return answer === undefined ? 'The terminal could not be opened. Open this session on claude.ai.' : answer.ok ? undefined : answer.message
              },
              onSend: async (id, message) => {
                const answer = await window.desktop?.sendClaudeCloud(id, message).catch(() => undefined)
                // The bridge failing mid-send leaves it unknown, not unsent (0.568).
                return answer === undefined ? 'Locust could not confirm whether that was sent. Look at the session on claude.ai before sending it again.' : answer.ok ? undefined : answer.message
              },
              onCheck: async (id) => (await window.desktop?.checkClaudeCloud(id).catch(() => undefined)) ?? { ok: false, message: 'Claude Code could not be reached to read it. See it on claude.ai.' },
              onApply: async (id) => {
                const answer = await window.desktop?.applyClaudeCloud(id).catch(() => undefined)
                return answer === undefined ? 'Its change could not be applied. Nothing was changed.' : answer.ok ? undefined : answer.message
              }
            }}
          />
        ) : sideChat !== undefined && screen === 'workroom' ? (
          <SideChat
            turns={sideChat.runIds.flatMap((runId) => {
              const run = runs.get(runId)
              return run === undefined
                ? []
                : [{
                    key: runId,
                    prompt: run.prompt,
                    events: run.events,
                    phase: run.phase,
                    ...(run.data?.missionId === undefined ? {} : { missionId: run.data.missionId }),
                    ...(run.error === undefined ? {} : { error: run.error }),
                    ...(run.startedAtIso === undefined ? {} : { startedAtIso: run.startedAtIso })
                  }]
            })}
            model={sideChat.model}
            workspacePath={workspacePath}
            onAsk={async (question) => {
              const bridge = window.desktop
              if (bridge === undefined) return 'Locust is not ready yet. Nothing was asked.'
              // Each follow-up is asked of the side chat's own latest answer, so it remembers itself.
              const last = sideChat.runIds.at(-1)
              const of = (last === undefined ? undefined : runs.get(last)?.data?.missionId) ?? sideChat.of
              const answer = await bridge.askOnTheSide(of, question, sideChat.runIds.length + 1).catch(() => undefined)
              if (answer === undefined) return 'The question could not be asked. The conversation is unchanged, and nothing was sent.'
              if (!answer.ok) return answer.message
              setSideChat((current) => (current === undefined ? current : { ...current, runIds: [...current.runIds, answer.data.runId] }))
              return undefined
            }}
            approvals={approvals.filter((request) => sideChat.runIds.includes(request.runId))}
            decidingIds={decidingIds}
            onDecide={decideApproval}
            onAnswerQuestion={answerQuestion}
            onStop={stopSideRun}
            onClose={() => {
              // An answer still going is stopped with the panel (R33): nobody is left reading it.
              for (const runId of sideChat.runIds) {
                const run = runs.get(runId)
                if (run !== undefined && liveRunIsActive(run)) stopSideRun(runId)
              }
              setSideChat(undefined)
            }}
          />
        ) : besideRun !== undefined && screen === 'workroom' ? (
          <BesideConversation
            run={besideRun}
            title={besideRow?.title ?? 'Conversation'}
            owner={teammates.find((entry) => entry.teammateId === ownerOf(besideRun))}
            teammates={teammates}
            workspacePath={workspacePath}
            onOpenHere={() => {
              const id = besideRun.data?.missionId
              setBesideId(undefined)
              if (id !== undefined) openMission(id)
            }}
            onClose={() => setBesideId(undefined)}
          />
        ) : changesOpen && changesAvailable && reviewing === undefined && screen === 'workroom' ? (
          /*
           * Notes on a line (0.376) in the folder's whole change too (0.776): they go, as in the conversation's own
           * diffs, to the conversation on screen. With none on screen there is nowhere to send one, so no "+".
           */
          <DiffNotesContext.Provider value={diffNotesPlace}>
          <ChangesPanel
            workspacePath={workspacePath}
            running={running}
            committed={commits}
            onClose={() => setChangesOpen(false)}
            {...((liveRun?.data?.missionId ?? liveRun?.restoredMission?.missionId) === undefined || liveRun === undefined
              ? {}
              : {
                  walkWith: {
                    of: (liveRun.data?.missionId ?? liveRun.restoredMission?.missionId)!,
                    teammate: pickedTeammate?.name ?? 'Your teammate',
                    asks: [...(liveRun.earlierTurns ?? []).map((turn) => turn.prompt), liveRun.prompt].map((prompt) => splitAttachments(prompt).text).slice(-5)
                  }
                })}
          />
          </DiffNotesContext.Provider>
        ) : reviewing !== undefined && screen === 'workroom' ? (
          // A teammate's branch takes notes only when it is that teammate's conversation on screen: a note goes there.
          <DiffNotesContext.Provider value={reviewing.teammateId === pickedTeammate?.teammateId ? diffNotesPlace : undefined}>
          <ReviewChanges
            key={reviewing.teammateId}
            teammate={reviewing}
            running={running}
            // The mode the composer shows is the one that will run (see the Composer's `mode`).
            canEdit={(() => {
              const shown = modeRunsOn(mode, composerRoute.runtime, build?.platform) ? mode : modesFor(composerRoute.runtime, build?.platform)[0] ?? mode
              return shown !== 'ask' && shown !== 'plan'
            })()}
            onAsk={startFromComposer}
            onClose={() => setReviewingId(undefined)}
          />
          </DiffNotesContext.Provider>
        ) : (
          inspectorOpen && liveRun !== undefined && screen === 'workroom' && (
            <Inspector
              events={liveRun.events}
              workspacePath={workspacePath}
              running={running}
              route={liveRun.data}
              /*
               * THE RECEIPT OF A RUN THAT FINISHED ON SCREEN.
               *
               * Only a conversation reopened from history carried its ledger
               * record, so the Receipt tab of a run that had just finished
               * said "the durable receipt appears once this mission has been
               * recovered from the ledger" and offered nothing -- the run was
               * over and its record was on disk. Found twice by the design
               * reviews (0.255 and 0.268: "a trust issue, not just copy").
               * The history is read again the moment a run ends, so its
               * record is the one the reopened view would have shown.
               */
              restoredMission={
                liveRun.restoredMission
                ?? (running || liveRun.data?.missionId === undefined ? undefined : historyById.get(liveRun.data.missionId))
              }
              onClose={() => setInspectorOpen(false)}
            />
          )
        )}
      </div>
      {sharingTeam && <ShareTeamDialog teammates={teammates} onClose={() => setSharingTeam(false)} />}
      {savingRecordOf !== undefined && <SaveRecordDialog missionId={savingRecordOf} onClose={() => setSavingRecordOf(undefined)} />}
      {paletteOpen && (
        <CommandPalette
          onClose={() => setPaletteOpen(false)}
          actions={
            [
              {
                id: 'go-workroom',
                // Its own group, because it is a promotion rather than an
                // accident of ordering: the workroom is the likeliest thing
                // anybody opening this wants, and a name says why it is
                // first (frame pass, 2026-09-15).
                group: 'This conversation',
                // "Workroom" was the screen's name in the code, not a word a
                // person has for it (first-impressions pass, 0.354).
                label: 'Back to the conversation',
                run: () => setScreen('workroom')
              },
              {
                id: 'toggle-swarm',
                group: 'Workspace',
                label: swarm ? 'Turn swarm off' : 'Turn swarm on',
                hint: 'every run at its model maximum',
                run: () => {
                  // Same write as the composer mark, which is the only other
                  // way in -- and it only exists on the workroom, and is
                  // disabled while a mission runs. A workspace-wide setting
                  // needs one way in from anywhere.
                  const next = !swarm
                  setSwarm(next)
                  if (next) setSwarmCalls((count) => count + 1)
                  void window.desktop
                    ?.writeWorkspaceSettings({ swarm: next, relay, relayHopCap, interrupt, memoryMode, autoMode, askConnectors, keepATodoList, replySize, layout, tube })
                    .then((settings) => setSwarm(settings.swarm === true))
                    .catch(() => setSwarm(!next))
                }
              },
              // In the order of their shortcuts (design pass, 0.715): listed 1, 4, 5, 2, 3, they read as unsorted.
              { id: 'go-missions', group: 'Go to', label: 'Conversations', hint: 'Ctrl 1', run: () => setScreen('missions') },
              {
                id: 'go-teammates',
                group: 'Go to',
                label: 'Team',
                hint: 'Ctrl 2',
                run: () => setScreen('teammates')
              },
              {
                id: 'go-settings',
                group: 'Go to',
                label: 'Settings',
                hint: 'Ctrl 3',
                // Same as the rail button: the storage number is read when
                // the screen that shows it opens, by every route in.
                run: () => {
                  refreshStorage()
                  setScreen('settings')
                }
              },
              { id: 'go-rooms', group: 'Go to', label: 'Rooms', hint: 'Ctrl 4', run: () => setScreen('rooms') },
              { id: 'go-memory', group: 'Go to', label: 'Memory', hint: 'Ctrl 5', run: () => setScreen('memory') },
              // Claude Code's /usage, for every agent (0.714's dialog), from anywhere.
              { id: 'go-usage', group: 'Go to', label: 'Usage', hint: 'every agent and model', run: () => setUsageOpen(true) },
              {
                id: 'new-teammate',
                group: 'Teammates',
                label: 'New teammate',
                run: () => {
                  setTeammateError(undefined)
                  setNewTeammateOpen(true)
                }
              },
              {
                id: 'inspector',
                group: 'Conversation',
                label: inspectorOpen ? 'Close the Activity panel' : 'Open the Activity panel',
                hint: 'Ctrl I',
                run: () => setInspectorOpen(!inspectorOpen)
              },
              ...(running
                ? [
                    {
                      id: 'stop',
                      group: 'Conversation',
                      label: 'Stop the running reply',
                      run: cancelMission
                    }
                  ]
                : []),
              // Only where a conversation is on screen: the record is of that one (0.575).
              ...(terminalMissionId === undefined || terminalMissionId.startsWith('pending:')
                ? []
                : [
                    {
                      id: 'save-record',
                      group: 'Conversation',
                      label: 'Save the record…',
                      hint: 'this conversation, as one file',
                      run: () => setSavingRecordOf(terminalMissionId)
                    }
                  ]),
              /*
               * The conversations, found by any word said in them or by the
               * teammate's name (0.414). The placeholder has always promised
               * "teammate or mission" and the palette listed none -- and in the
               * rail layout, under 1200px, the sidebar's search box is hidden,
               * so this was the only search there and it could not find a
               * conversation. The five newest show before anything is typed.
               */
              ...[...sidebarMissions]
                .sort((left, right) => Date.parse(right.lastAt ?? '') - Date.parse(left.lastAt ?? ''))
                .map((mission, position) => {
                  const ownerId = mission.ownerId ?? missionOwners[mission.missionId]
                  const owner = teammates.find((entry) => entry.teammateId === ownerId)?.name
                  const age = shortAgo(mission.lastAt)
                  const hint = [owner, age].filter((part): part is string => part !== undefined).join(' · ')
                  return {
                    id: `conversation:${mission.missionId}`,
                    group: 'Conversations',
                    label: mission.title,
                    ...(hint.length === 0 ? {} : { hint }),
                    keywords: `${mission.said ?? ''} ${owner ?? ''}`,
                    ...(position < 5 ? {} : { whenTyped: true }),
                    run: () => openMission(mission.missionId)
                  }
                })
            ] satisfies PaletteAction[]
          }
        />
      )}
      {screen === 'workroom' && <SelectionAsk onAsk={(quote) => setQuoteIn({ quote })} />}
      {usageOpen && window.desktop !== undefined && (
        <UsageDialog readUsage={(range) => window.desktop!.readUsage(range)} usageWindows={usageWindows} onClose={() => setUsageOpen(false)} />
      )}
      {importOpen && window.desktop !== undefined && (
        <ImportDialog
          onList={() => window.desktop!.listImportableSessions()}
          onImport={(session) => window.desktop!.importSession(session.runtime, session.sessionId)}
          onImported={(missionId) => {
            setImportOpen(false)
            openAfterImport.current = missionId
            refreshHistory()
            // Its title is the session's, kept with the roster's titles.
            void window.desktop?.listTeammates().then((listed) => {
              if (listed.ok) rememberRosterNames(listed.data)
            }).catch(() => undefined)
          }}
          onCancel={() => setImportOpen(false)}
        />
      )}
      {(() => {
        const instructing = groups.find((entry) => entry.groupId === instructingGroupId)
        return instructing === undefined ? null : (
          <GroupSettingsDialog
            key={instructing.groupId}
            group={instructing}
            // What "Use current" takes: the route a mission would start on
            // if you pressed send now, effort included.
            currentRoute={{ runtime: route.runtime, ...(route.account === undefined ? {} : { account: route.account }), model: route.model, mode, ...(effort === undefined ? {} : { effort }) }}
            onCancel={() => setInstructingGroupId(undefined)}
            onSave={({ instructions, route: nextRoute, routeChanged }) => {
              setInstructingGroupId(undefined)
              const writes: Promise<unknown>[] = []
              if (instructions !== instructing.instructions.trim()) {
                writes.push(window.desktop?.setGroupInstructions(instructing.groupId, instructions) ?? Promise.resolve())
              }
              if (routeChanged) writes.push(window.desktop?.setGroupRoute(instructing.groupId, nextRoute) ?? Promise.resolve())
              void Promise.all(writes).then(refreshGroups)
            }}
          />
        )
      })()}
      {splashOpen && (
        <WhatsNewSplash
          entries={splashEntries}
          onClose={() => setSplashClosed(true)}
          onSeeEverything={() => {
            setSplashClosed(true)
            setSettingsLanding('whatsnew')
            refreshStorage()
            setScreen('settings')
          }}
        />
      )}
      {tuning !== undefined && <TuneTeammate card={tuning} onChange={(edit) => setTuning((current) => current?.id !== tuning.id ? current : edit(current))} onCancel={closeTune} onRun={() => void runTune()} />}
      {newTeammateOpen && (
        <NewTeammateDialog
          error={teammateError}
          mode={mode}
          takenHues={teammates.map((teammate) => teammate.hue)}
          takenNames={teammates.map((teammate) => teammate.name)}
          onCancel={() => setNewTeammateOpen(false)}
          onCreate={createTeammate}
          composerRoute={{ runtime: route.runtime, ...(route.account === undefined ? {} : { account: route.account }), model: route.model, mode, ...(effort === undefined ? {} : { effort }) }}
          picker={{ runtimes, models, resolvedModels, recentRoutes, limitedRuntimes }}
          {...(build?.platform === undefined ? {} : { platform: build.platform })}
        />
      )}
      {editingTeammate !== undefined && (
        <NewTeammateDialog
          key={editingTeammate.teammateId}
          initial={editingTeammate}
          error={teammateError}
          mode={mode}
          takenNames={teammates.filter((teammate) => teammate.teammateId !== editingTeammate.teammateId).map((teammate) => teammate.name)}
          onCancel={() => {
            setFolderNotice(undefined)
            setEditingTeammate(undefined)
          }}
          onCreate={(input) => updateTeammate(editingTeammate.teammateId, input)}
          {...(spendByTeammate[editingTeammate.teammateId] === undefined ? {} : { spentThisMonth: spendByTeammate[editingTeammate.teammateId]! })}
          composerRoute={{ runtime: route.runtime, ...(route.account === undefined ? {} : { account: route.account }), model: route.model, mode, ...(effort === undefined ? {} : { effort }) }}
          picker={{ runtimes, models, resolvedModels, recentRoutes, limitedRuntimes }}
          {...(build?.platform === undefined ? {} : { platform: build.platform })}
          onChooseFolder={(clear) => chooseTeammateFolder(editingTeammate.teammateId, clear)}
          {...(connectorList === undefined ? {} : { connectors: connectorList })}
          onSetConnectors={(names) => setTeammateConnectors(editingTeammate.teammateId, names)}
          {...(folderNotice === undefined ? {} : { folderNotice })}
        />
      )}
      {feedbackFor !== undefined && (
        <FeedbackDialog conversation={feedbackFor.conversation} onClose={() => setFeedbackFor(undefined)} />
      )}
      {cloudSetup !== undefined && (
        <CloudSetupDialog
          state={cloudSetup}
          teammateName={pickedTeammate?.name}
          onSetUp={() => {
            const repo = 'repo' in cloudSetup ? cloudSetup.repo : undefined
            if (repo !== undefined && repo.length > 0) void setUpCloudComputer(repo, 'agent' in cloudSetup ? cloudSetup.agent : undefined)
          }}
          onSignIn={() => {
            if ('computer' in cloudSetup && cloudSetup.computer !== undefined && cloudSetup.agent !== undefined) void signInCloudAgent(cloudSetup.repo ?? '', cloudSetup.computer, cloudSetup.agent)
          }}
          onStopSignIn={() => void window.desktop?.stopCloudSignIn().catch(() => undefined)}
          onAllow={() => void window.desktop?.allowCloudComputers().catch(() => undefined)}
          onCheckAgain={() => void checkCloudComputer(composerRoute.runtime === 'claude' || composerRoute.runtime === 'codex' ? (computerOn ? composerRoute.runtime : undefined) : undefined)}
          onClose={() => {
            // Not set up or not signed in: Cloud goes back off, so a send cannot go nowhere.
            if (cloudSetup.phase !== 'ready' && cloudSetup.phase !== 'signed-in') {
              setCloudOn(false)
              setComputerOn(false)
            }
            setCloudSetup(undefined)
          }}
        />
      )}
      {routineRunFor !== undefined && <RoutineRunDialog routine={routineRunFor} onRun={(values) => startRoutine(routineRunFor.routineId, values)} onCancel={() => setRoutineRunFor(undefined)} />}
      {routineExport !== undefined && <RoutineExportDialog flagged={routineExport.flagged} onExport={(paths) => exportRoutine(routineExport.routineId, paths)} onCancel={() => setRoutineExport(undefined)} />}
      {templatePicker && <RoutineTemplateDialog templates={routineTemplates} onUse={openRoutineTemplate} onCancel={() => setTemplatePicker(false)} />}
      {routineImport !== undefined && <RoutineImportDialog preview={routineImport} fromTemplate={routineImport.fromTemplate === true} team={teammates} onCancel={() => setRoutineImport(undefined)} onImport={async (teammateId) => {
        if (!window.desktop) throw new Error('Desktop connection is unavailable.')
        const route = teammates.find((mate) => mate.teammateId === teammateId)?.route ?? { runtime: composerRoute.runtime, model: composerRoute.model, mode: 'ask' as const }
        const answer = await window.desktop.importRoutine({ token: routineImport.token, teammateId, route })
        if (!answer.ok) throw new Error(answer.error.message)
        setRoutineNotice(routineImport.fromTemplate === true ? `Added ${routineImport.name}. It has no schedule and has not run.` : 'Routine imported. It has no schedule and has not run.')
        await reloadRoutines()
      }} />}
      {routineDialog !== undefined && (
        <RoutineDialog
          key={routineDialog.routineId ?? 'new'}
          teammate={teammates.find((entry) => entry.teammateId === routineDialog.teammateId)}
          {...(routineDialog.teammateId === undefined && routineDialog.routineId === undefined
            ? { chooseFrom: teammates }
            : {})}
          initialName={routineDialog.name}
          {...(() => {
            // Its own folder when it has one; a new one is made in the folder open now (0.512).
            const own = routines.find((entry) => entry.routineId === routineDialog.routineId)?.workspaceId
            const folder = folders.find((entry) => entry.id === (own ?? (routineDialog.routineId === undefined ? workspaceId : undefined)))
            return folder === undefined ? {} : { folder: { name: folder.name, path: folder.path } }
          })()}
          initialSteps={routineDialog.steps}
          {...(routineDialog.inputs === undefined ? {} : { initialInputs: routineDialog.inputs })}
          initialSchedule={routineDialog.schedule}
          // A hand-off chain (0.435): who takes each step, from the whole team.
          team={teammates}
          {...(routineDialog.handOffs === undefined ? {} : { initialHandOffs: routineDialog.handOffs })}
          {...(routineDialog.stepRoles === undefined ? {} : { initialStepRoles: routineDialog.stepRoles })}
          truncated={routineDialog.truncated}
          editing={routineDialog.routineId !== undefined}
          fresh={routineDialog.routineId === undefined && routineDialog.learnedFrom.length === 0}
          initialInCopy={routineDialog.inCopy === true}
          {...(routineDialog.goalTries === undefined ? {} : { initialGoalTries: routineDialog.goalTries })}
          {...(checkCommand.trim().length === 0 ? {} : { checkCommand })}
          running={routineDialog.running === true}
          {...(() => {
            const mode = routineDialog.savedMode ?? routineDialog.route?.mode
            return mode === undefined ? {} : { modeName: modeFacts(mode).name, modeReadsOnly: mode === 'ask' || mode === 'plan' }
          })()}
          routeLabel={
            routineDialog.route === undefined
              ? (() => {
                  // A saved routine runs on its teammate's route NOW (0.493): say that one.
                  const now = teammates.find((entry) => entry.teammateId === routineDialog.teammateId)?.route
                  return now === undefined || routineDialog.routineId === undefined
                    ? undefined
                    : routeChrome(now.runtime, now.model, now.model === 'account-default' ? 'default' : modelDisplayName(now.runtime, now.model))
                })()
              : // The composer's spelling, as a room answer has it -- not the raw
                // id: "OpenCode / opencode/nemotron-3-ultra-free" (0.417).
                routeChrome(
                  routineDialog.route.runtime,
                  routineDialog.route.model,
                  routineDialog.route.model === 'account-default' ? 'default' : modelDisplayName(routineDialog.route.runtime, routineDialog.route.model)
                )
          }
          busy={routineDialog.busy}
          error={routineDialog.error}
          onCancel={() => setRoutineDialog(undefined)}
          onSave={saveRoutine}
        />
      )}
    </div>
    </ApprovalRuleContext.Provider>
    </OrganizationPolicyContext.Provider>
    </CallTodosContext.Provider>
  )
}
