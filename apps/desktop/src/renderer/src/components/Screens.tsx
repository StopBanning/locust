import { AgentAccountsSettings } from './AgentAccountsSettings.js'
import { Fragment, useEffect, useRef, useState } from 'react'
import { BOT_SIZE } from '../botSizes.js'
import { boardSectionOf, teamBoard } from '../teamBoard.js'
import { waitingLine } from '../homeTeamState.js'
import { isConversationWait, type NeedsYouItem } from '../needsYou.js'
import { durationText, runSpanMs } from '../missionView.js'
import { WhatsNew } from './WhatsNew.js'
import { RemoteControlSetting } from './RemoteControlSetting.js'
import { TalkVoice } from './TalkVoice.js'
import { TalkShortcutSetting } from './TalkShortcutSetting.js'
import { ScoutSetting } from './ScoutSetting.js'
import type { ScoutState } from '../../../shared/scout.js'
import type { PlanRunsOut } from '../../../shared/carry-on.js'
import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import { SKILL_RUNTIME_NAMES } from '../../../shared/skill-runtimes.js'
import { VoiceSettings } from './VoiceSettings.js'
import { LocustMcpSettings } from './LocustMcpSettings.js'
import { PhoneSettings } from './PhoneSettings.js'
import { ChatAppsSettings } from './ChatAppsSettings.js'
import { policyFeatureRefusal, policyModeAllowed } from '../../../shared/organization-policy.js'
import { ManagedPolicyNote, OrganizationPolicySettings, useOrganizationPolicy } from '../organizationPolicy.js'
import { AcpAgentsSettings } from './AcpAgentsSettings.js'
import { SETTINGS_PAGES, matchedHeadings, pageMatches } from '../settingsPages.js'
import { agentCapabilityHeading, agentCapabilityLines } from '../agentCapabilities.js'
import { AddConnector } from './AddConnector.js'
import { GitHubAccount } from './GitHubAccount.js'
import { ConnectorHealth } from './ConnectorHealth.js'
import type { SettingsPageId } from '../settingsPages.js'
import { CloudComputers } from './CloudComputers.js'
import { ChatCommands } from './ChatCommands.js'
import type { LoginItemState, RuntimeUpdatesState, MetalMotion, MetalPreset, MetalStrength, ProfileBackupResponse, ProfileRestorePreview, ProfileRestoreResponse } from '../../../shared/ipc.js'
import { backedUpLine, countsLine, folderName } from '../backupWords.js'
import { SUPPORT_ADDRESS } from '../../../shared/support.js'
import { DEFAULT_CHECK_FIXES, MAX_CHECK_FIXES } from '../../../shared/check-fix.js'
import { isConnectorAgent } from '../../../shared/connector-add.js'
import type { Spend } from '../../../shared/spend.js'
import { isOwnRoute, modelDisplayName, routeChrome, routeModelName } from '../routeName.js'
import type { ReactElement, ReactNode } from 'react'

import type {
  AppUpdateResponse,
  DiagnosticsReport,
  AppUpdateState,
  AppChangelog,
  MissionPruneResponse,
  PublicTrashedMission,
  TrashListResponse,
  TrashMutationResponse,
  PublicRecoveredMission,
  PublicRoutine,
  PublicRuntimeArtifact,
  PublicRuntimeStatus,
  PublicStorageReport,
  PublicTeammate,
  MemoryMode,
  LayoutPreference,
  PublicRuntimeSetup,
  PublicWorkspaceBrief,
  PublicWorktree, TubePreference, ReplyTextSize } from '../../../shared/ipc.js'
import { roleLabelOf } from '../../../shared/ipc.js'
import type { TeammateStatusView } from '../status.js'
import {
  facePresenceFor,
  formatBytes,
  integrationOf,
  missionPhaseView,
  listTone,
  modeLabel,
  prunePreviewSummary,
  routeRowStatus,
  runtimeListOrder
} from '../status.js'
import { CliArtifacts } from './CliArtifacts.js'
import { Bot } from './Bot.js'
import type { EyeGlyphs } from './Bot.js'
import { TeammateBot } from './TeammateBot.js'
import { FaceLegend } from './FaceLegend.js'
import { RuntimeMark } from './RuntimeMark.js'
import { UsageMeters } from './UsageMeters.js'
import { WhatLocustCanStop } from './WhatLocustCanStop.js'
import { WHAT_LOCUST_CAN_STOP_LEDE } from '../../../shared/what-locust-can-stop.js'
import { NETWORK_DOC_LINK } from '../../../shared/outbound-links.js'
import { keepCurrentNote, offersUpdate, updateLine } from '../agentUpdates.js'
import { Icon } from './Icon.js'
import { ThemeSetting } from './ThemeSetting.js'
import { costGlanceCell, costTotal, missionCost, monthSpendLine, sumCosts } from '../cost.js'
import { conversationsOf } from '../missionsList.js'
import { shortAgo } from '../railFlyout.js'
import { agoLabel, teammateWork } from '../teammateWork.js'
import { routineRunSummary, routineScheduleSummary, routineStepLabel } from '../routines.js'
import { RoutineRecovery } from './RoutineRecovery.js'
import type { RecoverRoutine } from './RoutineRecovery.js'
import { FREE_START_RUNTIME, installCommand } from '../../../shared/runtime-install.js'
import { SignInButton } from './SignInButton.js'
import { FeedbackDialog } from './FeedbackDialog.js'
import { WorktreeRow } from './WorktreeRow.js'
import type { WorktreeRemoval } from './WorktreeRow.js'
import { ArmedButton } from './ArmedButton.js'
import { OwnModels } from './OwnModels.js'
import { SkillsFromGitHub } from './SkillsFromGitHub.js'
import { routineAwaitsReview } from '../../../shared/routine-recovery.js'
import { SavedApprovalRules } from './SavedApprovalRules.js'

export type Screen = 'workroom' | 'missions' | 'teammates' | 'settings' | 'rooms' | 'memory' | 'automations' | 'board' | 'benchmarks'

/**
 * THE header for a list screen. Exported since 2026-09-22 because Routines
 * had a second one.
 *
 * Routines rendered `.lc-screen__head` -- a flex column with a UI-font lede
 * -- while Missions, Rooms and Memory rendered this 60px row. MEASURED on
 * the packaged build: 60 / 60 / 129px, so everything below the title stepped
 * down 69px on landing in Routines and back up on leaving. Colin: "why am I
 * so triggered". A tab click is the most frequent gesture in the app, and it
 * moved the page under him.
 *
 * One component, so a third form cannot appear by being written rather than
 * imported.
 */

/** Why the relay's own settings are off while it is. */
const RELAY_FIRST = 'This applies once teammates reply to each other. Switch that on first.'

export function ScreenHeader({ title, meta, actions }: { readonly title: string; readonly meta: string; readonly actions?: ReactNode }): ReactElement {
  return (
    <div className="lc-screen__header">
      <span className="lc-screen__title">{title}</span>
      <span className="lc-screen__meta lc-mono">{meta}</span>
      {actions !== undefined && <span className="lc-screen__actions">{actions}</span>}
    </div>
  )
}

/** Between a routine's steps in its hover text, so each is readable on its own line. */
const STEP_GAP = '\n\n'

const FILTERS = ['All', 'Running', 'Interrupted', 'Completed'] as const
type Filter = (typeof FILTERS)[number]

/**
 * A mission's phase for this screen, which has to account for the ones the
 * host is running RIGHT NOW.
 *
 * The ledger's own phase cannot say "running": a mission with no terminal
 * receipt reads as `interrupted`, which is the correct reading of a file but
 * the wrong word for a run that is still going. The shell knows which ids are
 * live, so it answers that here.
 */
export function missionRowPhase(
  mission: PublicRecoveredMission,
  runningMissionIds: ReadonlySet<string>
): PublicRecoveredMission['phase'] | 'running' {
  return runningMissionIds.has(mission.missionId) ? 'running' : mission.phase
}

export function matchesFilter(
  mission: PublicRecoveredMission,
  filter: Filter,
  runningMissionIds: ReadonlySet<string>
): boolean {
  if (filter === 'All') return true
  const phase = missionRowPhase(mission, runningMissionIds)
  if (filter === 'Running') return phase === 'running'
  if (filter === 'Interrupted') return phase === 'interrupted'
  return phase === 'completed'
}

/**
 * What the Missions header says when the ledger is not clean.
 *
 * Two different facts, and they must not be merged into one number: a mission
 * with an incomplete receipt is HERE and readable with a gap in it, while an
 * unreadable file is not here at all. "3 with an incomplete receipt" for two
 * of one and one of the other would replace a false reassurance with a false
 * count, which is not an improvement.
 *
 * Files, not issues: one truncated header raises both `truncated-tail` and
 * `invalid-record`, and "2 could not be read" for one file is its own lie.
 * `unreadableFileCount` in mission-history is what does that counting.
 */
export function ledgerDamageWords(withIssues: number, unreadable: number): string {
  const parts: string[] = []
  if (withIssues > 0) parts.push(`${String(withIssues)} with an incomplete receipt`)
  if (unreadable > 0) {
    parts.push(`${String(unreadable)} ${unreadable === 1 ? 'file' : 'files'} could not be read`)
  }
  return parts.join(' · ')
}

/**
 * Missions.
 *
 * The reference offers a "Needs approval" filter; there is no approval channel
 * yet, so a filter that can only ever return nothing is left out rather than
 * shipped as a dead control.
 */
export function MissionsScreen({
  workspaceId,
  missions,
  teammates,
  missionOwners,
  runningMissionIds,
  titleOf,
  secondaryOf,
  onOpen,
  unreadableLedgers = 0,
  ledgerUnreadable = false,
  totalMissions,
  listedMissions,
  onDeleteMissions,
  ownerId,
  onShowEveryone
}: {
  readonly missions: readonly PublicRecoveredMission[]
  /** The folder this window is open on, so the header can say how many are its own. */
  readonly workspaceId: string | undefined
  readonly teammates: readonly PublicTeammate[]
  readonly missionOwners: Readonly<Record<string, string>>
  /** The missions the host is running now; the ledger cannot know this. */
  readonly runningMissionIds: ReadonlySet<string>
  /**
   * The one line under the title: what this mission is doing right now, or
   * what it is waiting on you for.
   *
   * A list of twelve missions all reading COMPLETED or RUNNING tells you
   * which to open only by opening them. Undefined collapses the row to the
   * single line it has always been -- a settled mission has nothing live to
   * say, and inventing something for it would be noise on every row.
   */
  readonly secondaryOf?: (missionId: string) => string | undefined
  /**
   * What to call a mission. A continuation's own prompt can be the host's
   * briefing, and the sidebar already names such a row by the words a person
   * typed; this screen used to print the briefing in the same column as every
   * real instruction.
   */
  readonly titleOf: (mission: PublicRecoveredMission) => string
  readonly onOpen: (missionId: string) => void
  /**
   * Ledger files that raised an issue and produced no mission.
   *
   * They cannot appear in `missions` by definition -- that is the whole
   * defect: nothing in the list could carry their damage, so the header
   * reassured about them. Defaulted to zero so a caller that has not been
   * updated reads as it did before rather than crashing.
   */
  readonly unreadableLedgers?: number
  /**
   * The ledger could not be read at all, so this screen knows nothing.
   *
   * Distinct from "no missions": an empty ledger is a fact, and a ledger that
   * would not open is the absence of any facts. Both used to render the same
   * confident "ledger verified" -- seen by driving it with the ledger
   * directory replaced by a plain file (2026-09-08).
   */
  readonly ledgerUnreadable?: boolean
  /**
   * The ledger holds more turns than were listed (R28): said, so the count
   * is never read as a total and an older conversation is known to exist.
   */
  readonly totalMissions?: number
  readonly listedMissions?: number
  /**
   * Delete these records for good. Absent means the screen offers no
   * selection at all -- a list with checkboxes and nowhere to take them is
   * worse than a list without.
   */
  readonly onDeleteMissions?: (missionIds: readonly string[]) => void
  /**
   * Only this teammate's missions, as a teammate's card asks for (L23): its
   * "All of Wren's conversations" opened everyone's, or closed this screen.
   */
  readonly ownerId?: string
  /** Back to everyone's. */
  readonly onShowEveryone?: () => void
}): ReactElement {
  const [filter, setFilter] = useState<Filter>('All')
  const [picked, setPicked] = useState<ReadonlySet<string>>(new Set())
  const [armed, setArmed] = useState(false)
  const owner = ownerId === undefined ? undefined : teammates.find((teammate) => teammate.teammateId === ownerId)
  // One row per conversation, as the sidebar lists them (0.415): see missionsList.ts.
  const conversations = conversationsOf(missions)
  const ownerOfEntry = (entry: (typeof conversations)[number]): string | undefined =>
    entry.members.map((mission) => missionOwners[mission.missionId]).find((id) => id !== undefined)
  const shown = conversations.filter((entry) =>
    matchesFilter(entry.leaf, filter, runningMissionIds) && (ownerId === undefined || ownerOfEntry(entry) === ownerId))
  const withIssues = missions.filter((mission) => mission.integrityIssueCount > 0).length
  /*
   * "ledger verified" is a claim, and it must cover the files that are NOT here.
   *
   * `withIssues` counts recovered missions carrying an issue, which is every
   * mission whose file was damaged in its BODY. A file damaged in its HEADER,
   * or over the size limit, recovers no mission at all -- so it contributed
   * nothing to this count and the header said "0 local · ledger verified"
   * about a ledger the reader had just refused (Astra, 2026-09-08, rendering
   * this very component from hand-written damaged JSONL).
   *
   * The worse the damage, the more confident the reassurance. The count was
   * computed in mission-history and carried across IPC the whole time; nothing
   * on this side read it.
   */
  const damaged = withIssues + unreadableLedgers
  // This screen is the whole ledger; the sidebar is only the folder you are
  // in. Both are right and neither said so, so a tester counted 3 in one and
  // 5 in the other and could not tell which to believe (2026-09-07). Said
  // only when the two actually differ -- in one folder there is nothing to
  // reconcile and the extra clause would be noise.
  const elsewhere =
    workspaceId === undefined
      ? 0
      : conversations.filter((entry) => entry.leaf.workspaceId !== workspaceId).length
  // What the shown missions cost, in whatever units their receipts carry.
  // Runtimes that report nothing contribute nothing, and are counted as such
  // rather than as free. "priced" is a claim about money: when every receipt
  // reported tokens and no price -- which is what a free route gives -- the
  // honest word for the same count is "measured". See `costTotal`.
  const total = costTotal(shown.flatMap((entry) => entry.members.map((mission) => missionCost(mission))))

  /*
   * A running mission cannot be deleted -- the host refuses, because
   * deleting one would orphan a process still writing into a file that no
   * longer exists and take away the only control that stops it. So they are
   * not selectable here either: offering a checkbox for something that will
   * be refused is an offer the screen knows it cannot keep.
   */
  const deletable = shown.filter((entry) => !entry.members.some((mission) => runningMissionIds.has(mission.missionId)))
  const pickedHere = deletable.filter((entry) => picked.has(entry.key))
  const allPicked = deletable.length > 0 && pickedHere.length === deletable.length
  const toggle = (key: string): void => {
    setArmed(false)
    setPicked((current) => {
      const next = new Set(current)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  }
  const clear = (): void => {
    setPicked(new Set())
    setArmed(false)
  }

  return (
    <div className="lc-screen">
      <ScreenHeader
        title="Conversations"
        /*
         * "3 conversations", not "3 local": the count in the word a person
         * uses. And a total only when it is MONEY -- "54k in . 1.2k out across
         * 3 measured" was an engineer's unit (first-impressions pass, after
         * 0.349). The ledger's verdict stays: on this screen it is the claim
         * the tests below it guard, and it is never said when untrue.
         */
        // "all readable" said nothing on every healthy list (declutter, 0.723); a damaged record still says so.
        meta={`${conversations.length} ${conversations.length === 1 ? 'conversation' : 'conversations'}${elsewhere === 0 ? '' : `, ${conversations.length - elsewhere} in this folder`}${
          ledgerUnreadable
            ? ' · the records could not be read'
            : damaged === 0
              ? ''
              : ` · ${ledgerDamageWords(withIssues, unreadableLedgers)}`
        }${total === undefined || total.word !== 'priced' ? '' : ` · ${total.line} across ${String(total.runs)} priced`}${
          totalMissions === undefined || listedMissions === undefined ? '' : ` · only the newest ${listedMissions.toLocaleString('en-US')} of ${totalMissions.toLocaleString('en-US')} turns are listed`
        }`}
      />
      {owner !== undefined && (
        <div className="lc-filters" role="status">
          <button type="button" className="lc-filter is-active" aria-pressed="true" onClick={onShowEveryone}>
            Only {owner.name}’s · Show everyone’s
          </button>
        </div>
      )}
      {/*
        * Filters over an empty archive are four controls that can only ever
        * return nothing -- the same reason the approval filter is withheld
        * when there is nothing to approve. Raised by the design pass as a
        * feature question rather than a styling one, which it is.
        */}
      {missions.length > 0 && (
      <div className="lc-filters">
        {FILTERS.map((name) => (
          <button
            key={name}
            type="button"
            aria-pressed={filter === name}
            className={`lc-filter${filter === name ? ' is-active' : ''}`}
            onClick={() => setFilter(name)}
          >
            {name}
          </button>
        ))}
      </div>
      )}
      {/*
        * The selection bar, drawn only once something is selected.
        *
        * Colin, 2026-09-15: "can we add a check and delete, so we can delete
        * missions from here or delete more than once." Deleting one at a
        * time through a right-click menu is fine for one and absurd for
        * seventeen.
        *
        * Two steps, the same shape the row menu already uses: `Delete 6` and
        * then `Delete 6 for good?`. This removes records that cannot be
        * recovered, and a single click between a person and that is not
        * enough -- but a modal for it would be heavier than the act
        * deserves, and the menu settled that question already.
        */}
      {/*
        * L25 (the code review): the bar counted every pick, and Delete took
        * only the ones this filter shows -- "Delete 5" deleted 2, and the
        * other three were forgotten. It counts, and deletes, what is shown.
        */}
      {onDeleteMissions !== undefined && pickedHere.length > 0 && (
        <div className="lc-pickbar" role="status">
          <span className="lc-pickbar__count">
            {pickedHere.length} selected
          </span>
          <button type="button" className="lc-pickbar__link" onClick={clear}>
            Clear
          </button>
          <button
            type="button"
            className="lc-pickbar__link"
            onClick={() => {
              setArmed(false)
              setPicked(allPicked ? new Set() : new Set(deletable.map((entry) => entry.key)))
            }}
          >
            {allPicked ? 'Select none' : `Select all ${deletable.length}`}
          </button>
          <button
            type="button"
            className={`lc-pickbar__delete${armed ? ' is-armed' : ''}`}
            onClick={(event) => {
              if (!armed) {
                setArmed(true)
                return
              }
              // M34: the second click of a double-click is not a second decision.
              if (event.detail > 1) return
              // Only what is still selectable: a mission that started running
              // between the click and the confirm must not go.
              // A conversation goes with every turn, as the sidebar's Delete does.
              const goingKeys = deletable.map((entry) => entry.key).filter((key) => picked.has(key))
              const going = deletable
                .filter((entry) => picked.has(entry.key))
                .flatMap((entry) => entry.members.map((mission) => mission.missionId))
              onDeleteMissions(going)
              // Only the ones that went; a pick this filter hides stays picked.
              setPicked((current) => new Set([...current].filter((key) => !goingKeys.includes(key))))
              setArmed(false)
            }}
          >
            {armed ? `Delete ${pickedHere.length} for good?` : `Delete ${pickedHere.length}`}
          </button>
        </div>
      )}
      <div className="lc-screen__scroll">
        {shown.length === 0 ? (
          <p className="lc-inspector__empty">
            {missions.length === 0
              ? 'No conversations on this machine yet.'
              : 'No conversations match this filter.'}
          </p>
        ) : (
          <div className="lc-missionrows">
            {shown.map((entry) => {
              // The row stands for the conversation and reads as where it is now: its newest turn.
              const mission = entry.leaf
              /*
                * RUNNING comes from the host, not the ledger.
                *
                * `missionRowPhase` has existed for exactly this since the
                * screen was written, and only the FILTER ever called it --
                * the row read `mission.phase` straight, so a live mission
                * wore INTERRUPTED, which is the correct reading of a file
                * with no terminal receipt and the wrong word for a run that
                * is still going. It never showed, because until the live
                * runs were listed at all there was no row to be wrong
                * (MEASURED 2026-09-11: `INTERRUPTED || using a tool · Read`).
                */
              const view = missionPhaseView(
                missionRowPhase(mission, runningMissionIds),
                mission.integrityIssueCount > 0
              )
              const owner = teammates.find((teammate) => teammate.teammateId === ownerOfEntry(entry))
              const title = titleOf(entry.root)
              /*
               * ONE clock, `durationText`.
               *
               * This rounded to whole minutes, so a 41-second run read `0m`
               * here while the fold two inches away said `41s` -- the same
               * mission, timed twice, disagreeing. Grok's finding 2,
               * 2026-09-13, and the same shape as every other one in that
               * report: two surfaces computing one fact separately.
               */
              const elapsed = durationText(entry.members.reduce((sum, turn) =>
                sum + (runSpanMs(turn.events) ?? Math.max(0, Date.parse(turn.lastUpdatedAt) - Date.parse(turn.createdAt))), 0))
              const secondary = secondaryOf?.(mission.missionId)
              const running = entry.members.some((turn) => runningMissionIds.has(turn.missionId))
              const isPicked = picked.has(entry.key)
              return (
                <div
                  key={entry.key}
                  className={`lc-missionrowwrap${isPicked ? ' is-picked' : ''}`}
                >
                  {/*
                    * Beside the row, never inside it. The row is a button and
                    * a checkbox nested in one is neither valid nor reachable
                    * -- the same reason the sidebar's conversation row wraps
                    * its `...` menu rather than nesting it.
                    */}
                  {onDeleteMissions !== undefined && (
                    <input
                      type="checkbox"
                      className="lc-missionrow__pick"
                      checked={isPicked}
                      disabled={running}
                      aria-label={running ? `${title} is still running` : `Select ${title}`}
                      title={running ? 'Still running — stop it first' : undefined}
                      onChange={() => toggle(entry.key)}
                    />
                  )}
                <button
                  type="button"
                  className="lc-missionrow"
                  onClick={() => onOpen(mission.missionId)}
                >
                  <span className={`lc-rail__dot lc-tone-${listTone(view)}`} />
                  {/*
                    * Title, and under it the live word.
                    *
                    * `Pending: …` on a mission waiting for you is the whole
                    * point: amber already says a person may need to act, and
                    * this says what for, without opening it.
                    */}
                  <span className="lc-missionrow__name">
                    <span className="lc-missionrow__title">{title}</span>
                    {secondary !== undefined && <span className="lc-missionrow__doing">{secondary}</span>}
                  </span>
                  {/* When: every row said who and on what, and none said when (0.415). */}
                  <span className="lc-missionrow__when lc-mono" title={new Date(mission.lastUpdatedAt).toLocaleString()}>
                    {shortAgo(mission.lastUpdatedAt) ?? ''}
                  </span>
                  <span className="lc-missionrow__owner">{owner?.name ?? '—'}</span>
                  <span className="lc-missionrow__route lc-mono">
                    {/* The route, spelled the way the composer and sidebar
                        spell it. This printed raw ids -- `opencode /
                        muse-spark-1.3-contributor-free` -- so one route read
                        three different ways depending on the screen (Grok's
                        finding 1). Receipts keep raw ids on purpose; a row is
                        not a receipt. */}
                    {!isOwnRoute(mission.model) && <RuntimeMark runtime={mission.runtime} size={11} className="is-inline" />}
                    {routeChrome(mission.runtime, mission.model, modelDisplayName(mission.runtime, mission.model))}
                  </span>
                  <span className="lc-missionrow__stats lc-mono">
                    {/* A count of none says nothing (Yurt's beta report, #14). */}
                    {entry.members.length > 1
                      ? `${String(entry.members.length)} turns · ${elapsed}`
                      : mission.checkpoints.length === 0
                        ? elapsed
                        : `${String(mission.checkpoints.length)} checkpoint${mission.checkpoints.length === 1 ? '' : 's'} · ${elapsed}`}
                  </span>
                  <span className="lc-missionrow__cost lc-mono" title="What the runtime reported this run cost">
                    {costGlanceCell(sumCosts(entry.members.map((turn) => missionCost(turn))), mission.model)}
                  </span>
                  <span className={`lc-missionrow__tag lc-mono lc-tone-${listTone(view)}`}>{view.tag}</span>
                </button>
                </div>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}

/** Nobody's finish is waiting, when the shell passes none. */
const NONE_UNSEEN: ReadonlySet<string> = new Set()

export function TeammatesScreen({
  teammates,
  missions,
  missionOwners,
  spendByTeammate,
  viewByTeammate,
  titleOf,
  onOpenMission,
  onNewTeammate,
  onEdit,
  onRemove,
  onMessage,
  routines,
  routineStepByTeammate,
  onRunRoutine,
  onRecoverRoutine,
  onEditRoutine,
  onRemoveRoutine,
  routineNotice,
  onDismissRoutineNotice,
  onShareTeam,
  onAddTeamFromCard,
  teamNotice,
  finishedUnseen,
  runningMissionIds,
  needsYou = [],
  droppedTeammates
}: {
  /** Teammates on a free model OpenCode no longer lists (0.819, status.droppedFreeModel): said on their cards. */
  readonly droppedTeammates?: ReadonlySet<string>
  /**
   * Missions with a live run in this window. The record calls a mission that
   * has not ended `interrupted` -- it has no word for running -- so a live job
   * drew a red dot in Recent (0.380, seen on the board). The Missions screen
   * already takes this; the Team screen now does too.
   */
  readonly runningMissionIds?: ReadonlySet<string>
  /** The same waits Home reads, in the same order: approvals before questions. */
  readonly needsYou?: readonly NeedsYouItem[]
  /** 0.380: teammates whose run ended while nobody was looking -- the board's "Just finished". */
  readonly finishedUnseen?: ReadonlySet<string>
  /** M30: why a routine's Run did not start, said where it was pressed. */
  readonly routineNotice?: string
  readonly onDismissRoutineNotice?: () => void
  /** Open the team card to save (0.398). */
  readonly onShareTeam?: () => void
  /** What adding a team from an image did, said on this screen (0.398). */
  readonly teamNotice?: string
  /** Pick a team card and add its teammates (0.398). */
  readonly onAddTeamFromCard?: () => void
  readonly teammates: readonly PublicTeammate[]
  /** Every recovered mission, so a card can say what its teammate has done. */
  readonly missions: readonly PublicRecoveredMission[]
  readonly missionOwners: Readonly<Record<string, string>>
  /** Each teammate's money this month, from the host's read of every conversation. */
  readonly spendByTeammate: Readonly<Record<string, Spend>>
  /** The words a person typed, which for a continuation is not its own prompt. */
  readonly titleOf: (mission: PublicRecoveredMission) => string
  readonly onOpenMission: (missionId: string) => void
  /** Each teammate's face state, decided once in the shell so the roster agrees with the sidebar. */
  readonly viewByTeammate: Readonly<Record<string, TeammateStatusView>>
  readonly onNewTeammate: () => void
  readonly onEdit: (teammate: PublicTeammate) => void
  readonly onRemove: (teammateId: string) => void
  /** Their conversation, as a face in the rail opens it. */
  readonly onMessage: (teammateId: string) => void
  /** Every routine on file. Each is filed under the teammate that runs it. */
  readonly routines: readonly PublicRoutine[]
  /** Which routine each teammate is replaying right now, if any. */
  readonly routineStepByTeammate: Readonly<Record<string, { readonly name: string; readonly step: number; readonly of: number }>>
  readonly onRunRoutine: (routineId: string) => void
  readonly onRecoverRoutine?: RecoverRoutine
  readonly onEditRoutine: (routine: PublicRoutine) => void
  readonly onRemoveRoutine: (routineId: string) => void
}): ReactElement {
  // One card per teammate, the same in the plain grid and in each board section.
  const card = (teammate: PublicTeammate): ReactElement => {
    const owned = Object.values(missionOwners).filter((owner) => owner === teammate.teammateId).length
    const work = teammateWork(teammate.teammateId, missions, missionOwners, titleOf)
    const month = monthSpendLine(spendByTeammate[teammate.teammateId], teammate.monthlyLimitUsd)
    const theirRoutines = routines.filter((routine) => routine.teammateId === teammate.teammateId)
    const replaying = routineStepByTeammate[teammate.teammateId]
    const waiting = needsYou.find((item): item is Extract<NeedsYouItem, { kind: 'approval' | 'decision' }> => isConversationWait(item) && item.teammateId === teammate.teammateId)
    const question = waiting === undefined ? undefined : waitingLine(waiting)
    return (
      <div className={`lc-rostercard lc-rostercard--${boardSectionOf(viewByTeammate[teammate.teammateId], finishedUnseen?.has(teammate.teammateId) === true)}`} key={teammate.teammateId}>
        <div className="lc-rostercard__head">
          <TeammateBot
            hue={teammate.hue}
            avatar={teammate.avatar}
            size={BOT_SIZE.rosterCard}
            activity={viewByTeammate[teammate.teammateId]?.activity ?? 'idle'}
            // The roster had no dot because it had no status. It has
            // both now, from the same call the sidebar reads.
            presence={facePresenceFor(viewByTeammate[teammate.teammateId]?.status ?? 'idle')}
            teammateId={teammate.teammateId}
          />
          <div className="lc-rostercard__id">
            <div className="lc-rostercard__name">{teammate.name}</div>
            <div className="lc-rostercard__role">{roleLabelOf(teammate)}</div>
          </div>
          {/*
            * ICONS, as the Routines row draws the same two actions.
            *
            * "Edit" and "Remove" as words took the width beside the
            * name, so a role like "Code & Migrations" wrapped to two
            * lines and cards in one row stood at different heights,
            * their stat boxes out of line (design pass and beta review
            * of 0.255.0, #4). Both carry a title and an accessible name.
            */}
          <div className="lc-rostercard__actions">
            {/*
              * The rail draws four faces once the team is six or more,
              * and its +N opens this screen -- which had no way to
              * talk to anyone on it. A fifth teammate could be edited
              * and removed, never messaged (found fixing the drives'
              * one way to open a teammate, 2026-09-23).
              */}
            <button
              type="button"
              className="lc-ghostbutton lc-iconbutton lc-rostercard__message"
              title="Message teammate"
              aria-label={`Message ${teammate.name}`}
              onClick={() => onMessage(teammate.teammateId)}
            >
              <Icon name="message" size={14} />
            </button>
            <button
              type="button"
              className="lc-ghostbutton lc-iconbutton lc-rostercard__edit"
              title="Edit teammate"
              aria-label={`Edit ${teammate.name}`}
              onClick={() => onEdit(teammate)}
            >
              <Icon name="pencil" size={14} />
            </button>
            {/* H2: asks first, as the right-click Remove does. */}
            <ArmedButton
              className="lc-ghostbutton lc-iconbutton lc-rostercard__remove"
              title="Remove teammate"
              ariaLabel={`Remove ${teammate.name}`}
              armedLabel={theirRoutines.length === 0 ? `Remove ${teammate.name}?` : `Remove, with ${String(theirRoutines.length)} ${theirRoutines.length === 1 ? 'routine' : 'routines'}?`}
              onConfirm={() => onRemove(teammate.teammateId)}
            >
              <Icon name="close" size={14} />
            </ArmedButton>
          </div>
        </div>
        {/*
          * Route and mode are identity, not status, and the model name
          * is the fact this product exists to keep legible -- so it
          * gets a full-width row and wraps rather than ever being cut.
          */}
        {/* No fixed model: no row. A row saying so sat on every such card and said nothing (declutter, 0.723). */}
        {teammate.route !== undefined && (
          <div className="lc-rostercard__route lc-mono">
            {/* A free model OpenCode dropped reads amber, and its hover says what to do (0.819), as on Home. */}
            <span className={`lc-rostercard__model${droppedTeammates?.has(teammate.teammateId) === true ? ' is-dropped' : ''}`}
              {...(droppedTeammates?.has(teammate.teammateId) === true ? { title: `OpenCode no longer offers ${routeModelName(teammate.route.runtime, teammate.route.model)}. Message ${teammate.name} to switch.` } : {})}>
              {!isOwnRoute(teammate.route.model) && <RuntimeMark runtime={teammate.route.runtime} size={12} className="is-inline" />}
              {routeChrome(teammate.route.runtime, teammate.route.model, routeModelName(teammate.route.runtime, teammate.route.model))}
            </span>
            <span>{modeLabel(teammate.route.mode)}</span>
          </div>
        )}
        {/* Home's words, with a way to the exact turn that wants an answer,
            not the teammate's most recently opened conversation. */}
        {waiting !== undefined && question !== undefined && (
          <button type="button" className="lc-rostercard__waiting" onClick={() => onOpenMission(waiting.missionId)}>
            <span className="lc-rostercard__question" title={question}>{question}</span>
            <span className="lc-rostercard__answer">Answer <Icon name="chevron-right" size={13} /></span>
          </button>
        )}
        <dl className="lc-rostercard__stats">
          <div className="lc-rostercard__stat">
            <dt>Runs</dt>
            <dd className={owned === 0 ? 'is-unreported' : undefined}>{owned}</dd>
          </div>
          <div className="lc-rostercard__stat">
            <dt>Last run</dt>
            <dd className={work.lastRunAt === undefined ? 'is-unreported' : undefined}>
              {work.lastRunAt === undefined ? 'never' : agoLabel(work.lastRunAt) ?? 'unknown'}
            </dd>
          </div>
        </dl>
        {/* Undefined is not zero: a runtime that reported no usage has
          * not said the work was free, and `not reported` must never
          * be truncated into saying something else. */}
        {/* Only once there has been a run to report on: before the first,
            "not reported by the runtime" blamed a runtime nobody had
            used yet (design pass, 2026-09-22). */}
        {/*
          * MONEY, OR NO ROW (first-impressions pass, after 0.349). The
          * card said "USAGE 18k in . 612 out" for every teammate -- an
          * engineer's unit on the card a person reads to know who is on
          * their team. What a teammate COST is worth a row; a token
          * count is in their conversations' Details. No row claims
          * nothing, so a run that reported no price is never shown as
          * free.
          *
          * THIS MONTH, from the host (2026-09-26). The row added up the
          * conversations the window held events for -- the newest
          * twenty -- so older work fell out of a figure labelled as the
          * whole. The host reads every conversation, and this month is
          * the figure a limit is set against.
          */}
        {month !== undefined && (
          <dl className={`lc-rostercard__cost${month.reached ? ' is-reached' : ''}`}>
            <dt>{month.reached ? 'Limit reached' : 'This month'}</dt>
            <dd className="lc-mono">{month.text}</dd>
            {month.unpriced !== undefined && <dd className="lc-rostercard__unpriced" title={month.note}>{month.unpriced}</dd>}
          </dl>
        )}
        {work.recent.length > 0 && (
          <div className="lc-rostercard__recent">
            <div className="lc-rostercard__recentlabel lc-mono">Recent</div>
            {work.recent.map((entry) => (
              <button
                type="button"
                key={entry.missionId}
                className="lc-rostercard__mission"
                onClick={() => onOpenMission(entry.missionId)}
              >
                <span className={`lc-dot lc-tone-${runningMissionIds?.has(entry.missionId) === true ? missionPhaseView('running', false).tone : listTone(missionPhaseView(entry.phase, entry.hasIntegrityIssues))}`} />
                <span className="lc-rostercard__missiontitle">{entry.title}</span>
                <span className="lc-rostercard__missionage lc-mono">{agoLabel(entry.at) ?? ''}</span>
              </button>
            ))}
          </div>
        )}
        {/*
          * Routines: conversations this teammate has been taught, and
          * can replay. Shown here rather than in a screen of their own
          * because a routine belongs to a teammate -- it runs on their
          * route, with their permissions, and dies with them.
          */}
        {theirRoutines.length > 0 && (
          <div className="lc-routinelist">
            <div className="lc-rostercard__recentlabel lc-mono">Routines</div>
            {theirRoutines.map((routine) => (
              <div className="lc-routinerow" key={routine.routineId}>
                <span className="lc-routinerow__name" title={routine.steps.join(STEP_GAP)}>
                  {routine.name}
                  <span className="lc-routinerow__meta lc-mono"> · {routineRunSummary(routine)}</span>
                  <RoutineRecovery key={`${routine.execution?.attemptId}:${routine.execution?.step}`} routine={routine} onOpenMission={onOpenMission}
                    {...(onRecoverRoutine === undefined ? {} : { recover: onRecoverRoutine })} />
                  {routineScheduleSummary(routine, new Date()) !== undefined && (
                    <span className="lc-routinerow__meta lc-mono lc-routinerow__sched">
                      {routineScheduleSummary(routine, new Date())}
                    </span>
                  )}
                </span>
                <button
                  type="button"
                  className="lc-ghostbutton"
                  disabled={replaying !== undefined || (routine.execution !== undefined && routine.execution.status !== 'abandoned')}
                  title={
                    replaying === undefined
                      ? undefined
                      : `${replaying.name} is running: ${routineStepLabel(replaying)}`
                  }
                  onClick={() => onRunRoutine(routine.routineId)}
                >
                  Run
                </button>
                <span className="lc-routinerow__meta">
                  <button type="button" className="lc-ghostbutton" onClick={() => onEditRoutine(routine)}>
                    Edit
                  </button>
                  <ArmedButton
                    className="lc-ghostbutton"
                    ariaLabel={`Remove ${routine.name}`}
                    armedLabel="Remove for good?"
                    {...(routineAwaitsReview(routine) ? { disabled: true, title: 'Waiting for your review: check it and abandon it before removing it' } : {})}
                    onConfirm={() => onRemoveRoutine(routine.routineId)}
                  >
                    Remove
                  </ArmedButton>
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    )
  }
  const newTeammate = (
    <button type="button" className="lc-rostercard lc-rostercard--new" onClick={onNewTeammate}>
      <span className="lc-rostercard__plus">+</span>
      <span className="lc-rostercard__name">New teammate</span>
      <span className="lc-rostercard__role">Name, role and avatar. Their conversations group under them.</span>
    </button>
  )
  // 0.380: grouped by what each needs from the person, or the plain grid for a quiet team.
  const board = teamBoard(teammates, viewByTeammate, finishedUnseen ?? NONE_UNSEEN)
  return (
    <div className="lc-screen">
      <ScreenHeader
        title="Team"
        meta={`${teammates.length} teammate${teammates.length === 1 ? '' : 's'}`}
        actions={
          /* The team as a picture of itself, out and in (0.398; TeamCard.tsx). */
          <>
            {onAddTeamFromCard !== undefined && (
              <button type="button" className="lc-ghostbutton" onClick={onAddTeamFromCard} title="Add the teammates on a Locust team card">
                Add team from image
              </button>
            )}
            {onShareTeam !== undefined && teammates.length > 0 && (
              <button type="button" className="lc-ghostbutton" onClick={onShareTeam} title="Save your team as an image anyone with Locust can add">
                Share team
              </button>
            )}
          </>
        }
      />
      {teamNotice !== undefined && <p className="lc-claim lc-claim--hint lc-teamnotice" role="status">{teamNotice}</p>}
      {routineNotice !== undefined && (
        <p className="lc-claim lc-claim--hint lc-tone-amber" role="alert">
          {routineNotice}{' '}
          <button type="button" className="lc-ghostbutton" onClick={onDismissRoutineNotice}>
            Dismiss
          </button>
        </p>
      )}
      <div className="lc-screen__scroll">
        {board === undefined ? (
          <div className="lc-rostergrid">
            {teammates.map(card)}
            {newTeammate}
          </div>
        ) : (
          <div className="lc-teamboard">
          {board.map((section, index) => (
            <section key={section.key} className={`lc-boardsection lc-boardsection--${section.key}`} aria-label={section.title}>
              <h2 className="lc-boardsection__title">
                <span className="lc-boardsection__mark" aria-hidden="true" />
                {section.title}
                <span className="lc-boardsection__count lc-mono">{section.members.length}</span>
              </h2>
              <div className="lc-rostergrid">
                {section.members.map(card)}
                {index === board.length - 1 ? newTeammate : null}
              </div>
            </section>
          ))}
          </div>
        )}
      </div>
    </div>
  )
}

/**
 * Settings.
 *
 * Runtimes read from discovery. Fallback policy and swarm are drawn because the
 * design places them here, but both are shown as unavailable with the reason --
 * fallback needs route switching, and swarm needs a route that reports whether
 * it honours effort. Neither is rendered as a working toggle that does nothing.
 */
/** The ages offered. Long by default: history is the point of the ledger. */
const RETENTION_CHOICES = [30, 90, 365] as const

/**
 * What has been deleted and is still here.
 *
 * Deleting takes a conversation out of every listing at once, which is what a
 * person means by it. The record itself is kept until this panel is emptied,
 * so the one thing the app could not do on 2026-09-17 -- give an accidentally
 * deleted day of work back -- it can now do.
 *
 * Restoring is one press, because the case it exists for is a mistake that has
 * already happened. Emptying is two, differently worded, because that is the
 * press that cannot be taken back.
 */
function TrashControl({
  onList,
  onRestore,
  onEmpty
}: {
  readonly onList: () => Promise<TrashListResponse>
  readonly onRestore: (missionId: string) => Promise<TrashMutationResponse>
  readonly onEmpty: () => Promise<TrashMutationResponse>
}): ReactElement {
  const [held, setHeld] = useState<readonly PublicTrashedMission[] | undefined>(undefined)
  const [armed, setArmed] = useState(false)
  const [message, setMessage] = useState<string | undefined>(undefined)

  const load = (): void => {
    void onList()
      .then((response) => {
        setHeld(response.ok ? response.data.missions : [])
        if (!response.ok) setMessage(response.error.message)
      })
      .catch(() => setHeld([]))
  }
  useEffect(load, [])

  const restore = (missionId: string): void => {
    setMessage(undefined)
    void onRestore(missionId)
      .then((response) => {
        setMessage(response.ok ? 'Put back.' : response.error.message)
        load()
      })
      .catch(() => setMessage('That conversation could not be put back.'))
  }

  const empty = (event?: { readonly detail: number }): void => {
    if (!armed) {
      setArmed(true)
      return
    }
    // M34: the second click of a double-click is not a second decision.
    if ((event?.detail ?? 0) > 1) return
    setArmed(false)
    setMessage(undefined)
    void onEmpty()
      .then((response) => {
        setMessage(response.ok ? `Deleted ${String(response.data.count)} for good.` : response.error.message)
        load()
      })
      .catch(() => setMessage('The trash was not emptied.'))
  }

  const count = held?.length ?? 0
  const NEWLINE = String.fromCharCode(10)
  return (
    <div className="lc-trash">
      <p className="lc-settings__note">
        {held === undefined
          ? 'Reading the trash…'
          : count === 0
            ? 'Nothing deleted. A conversation you delete waits here until you empty it.'
            : `${String(count)} deleted conversation${count === 1 ? '' : 's'}, kept until you empty this.`}
      </p>
      {held !== undefined && count > 0 && (
        <>
          <ul className="lc-trash__list">
            {held.map((mission) => (
              <li className="lc-trash__row" key={mission.missionId}>
                <span className="lc-trash__what">
                  {mission.prompt === undefined || mission.prompt.trim().length === 0
                    ? mission.missionId
                    : mission.prompt.split(NEWLINE)[0]}
                </span>
                <span className="lc-trash__when lc-mono">{mission.deletedAt.slice(0, 16).replace('T', ' ')}</span>
                <button type="button" className="lc-button" onClick={() => restore(mission.missionId)}>
                  Put back
                </button>
              </li>
            ))}
          </ul>
          <button type="button" className="lc-button lc-button--danger" onClick={empty}>
            {armed ? `Delete ${String(count)} for good?` : 'Empty the trash'}
          </button>
        </>
      )}
      {message !== undefined && <p className="lc-settings__note">{message}</p>}
    </div>
  )
}

/**
 * Deleting old missions in bulk, in two deliberate steps.
 *
 * Nothing is ever pruned automatically, and the first press only ASKS. What
 * comes back is the host's own plan, computed by the same code that does the
 * deleting, and it names what would be kept as well as what would go. Only
 * then does a second, differently-worded press carry it out.
 */
function RetentionControl({
  report,
  onPreview,
  onPrune
}: {
  readonly report: PublicStorageReport | undefined
  readonly onPreview: (days: number) => Promise<MissionPruneResponse>
  readonly onPrune: (days: number) => Promise<MissionPruneResponse>
}): ReactElement {
  const [days, setDays] = useState<number>(90)
  const [state, setState] = useState<
    | { readonly kind: 'idle' }
    | { readonly kind: 'working' }
    | { readonly kind: 'preview'; readonly days: number; readonly summary: string; readonly count: number }
    | { readonly kind: 'done'; readonly summary: string }
    | { readonly kind: 'error'; readonly message: string }
  >({ kind: 'idle' })

  const ask = async (chosen: number): Promise<void> => {
    setState({ kind: 'working' })
    const response = await onPreview(chosen)
    if (!response.ok) {
      setState({ kind: 'error', message: response.error.message })
      return
    }
    setState({
      kind: 'preview',
      days: chosen,
      summary: prunePreviewSummary(response.data),
      count: response.data.deleted.length
    })
  }

  const confirm = async (chosen: number): Promise<void> => {
    setState({ kind: 'working' })
    const response = await onPrune(chosen)
    if (!response.ok) {
      setState({ kind: 'error', message: response.error.message })
      return
    }
    const gone = response.data.deleted.length
    setState({
      kind: 'done',
      summary: `Deleted ${String(gone)} turn${gone === 1 ? '' : 's'}.`
    })
  }

  return (
    <div className="lc-retention">
      <div className="lc-retention__row">
        <span className="lc-settings__note">Delete finished turns older than</span>
        {RETENTION_CHOICES.map((choice) => (
          <button
            key={choice}
            type="button"
            className={`lc-chip${choice === days ? ' is-on' : ''}`}
            aria-pressed={choice === days}
            onClick={() => {
              setDays(choice)
              setState({ kind: 'idle' })
            }}
          >
            {choice === 365 ? '1 year' : `${String(choice)} days`}
          </button>
        ))}
        <button
          type="button"
          className="lc-button"
          disabled={state.kind === 'working' || report?.missionCount === 0}
          onClick={() => void ask(days)}
        >
          {state.kind === 'working' ? 'Working…' : 'Review'}
        </button>
      </div>
      {state.kind === 'preview' && (
        <div className="lc-retention__plan">
          <span className="lc-settings__note">{state.summary}</span>
          {state.count > 0 && (
            <button type="button" className="lc-button lc-button--danger" onClick={() => void confirm(state.days)}>
              Delete them for good
            </button>
          )}
        </div>
      )}
      {state.kind === 'done' && <span className="lc-settings__note">{state.summary}</span>}
      {state.kind === 'error' && <span className="lc-settings__note lc-tone-red">{state.message}</span>}
    </div>
  )
}

/**
 * BACK UP AND RESTORE (0.614, main/profile-backup.ts; the PRD's R21).
 *
 * A backup is a new folder inside the one picked. A restore is read first, by
 * the same reader the restore uses, and shown: what the backup holds, what it
 * replaces. Only then is it asked for, and Locust restarts to apply it before
 * anything is read. What it replaces is moved aside, never deleted.
 */
function BackupControl({
  onPick,
  onBackUp,
  onPreview,
  onRestore
}: {
  readonly onPick: (purpose: 'backup' | 'restore') => Promise<string | undefined>
  readonly onBackUp: (folder: string) => Promise<ProfileBackupResponse>
  readonly onPreview: (folder: string) => Promise<ProfileRestorePreview>
  readonly onRestore: (folder: string) => Promise<ProfileRestoreResponse>
}): ReactElement {
  const [state, setState] = useState<
    | { readonly kind: 'idle' }
    | { readonly kind: 'working'; readonly what: 'backup' | 'reading' | 'restoring' }
    | { readonly kind: 'backed-up'; readonly line: string }
    | { readonly kind: 'preview'; readonly preview: Extract<ProfileRestorePreview, { readonly ok: true }> }
    | { readonly kind: 'restarting' }
    | { readonly kind: 'error'; readonly message: string }
  >({ kind: 'idle' })
  const backUp = async (): Promise<void> => {
    const folder = await onPick('backup')
    if (folder === undefined) return
    setState({ kind: 'working', what: 'backup' })
    const result = await onBackUp(folder)
    setState(result.ok ? { kind: 'backed-up', line: backedUpLine(result, formatBytes(result.bytes)) } : { kind: 'error', message: result.reason })
  }
  const read = async (): Promise<void> => {
    const folder = await onPick('restore')
    if (folder === undefined) return
    setState({ kind: 'working', what: 'reading' })
    const preview = await onPreview(folder)
    setState(preview.ok ? { kind: 'preview', preview } : { kind: 'error', message: preview.reason })
  }
  const restore = async (folder: string): Promise<void> => {
    setState({ kind: 'working', what: 'restoring' })
    const answer = await onRestore(folder)
    setState(answer.ok ? { kind: 'restarting' } : { kind: 'error', message: answer.reason })
  }
  const busy = state.kind === 'working' || state.kind === 'restarting'
  const doing = state.kind === 'working' ? state.what : undefined
  return (
    <div className="lc-backup">
      <div className="lc-backup__row">
        <button type="button" className="lc-button" disabled={busy} onClick={() => void backUp()}>
          {doing === 'backup' ? 'Backing up…' : 'Back up…'}
        </button>
        <button type="button" className="lc-button" disabled={busy} onClick={() => void read()}>
          {doing === 'reading' ? 'Reading the backup…' : 'Restore from a backup…'}
        </button>
      </div>
      {state.kind === 'backed-up' && <span className="lc-settings__note">{state.line}</span>}
      {state.kind === 'preview' && (
        <div className="lc-backup__plan">
          <dl className="lc-receipt lc-receipt--flush">
            <dt>Backup</dt>
            <dd className="lc-mono">{folderName(state.preview.folder)}</dd>
            <dt>Made</dt>
            <dd>
              {new Date(state.preview.createdAt).toLocaleString()} by Locust {state.preview.appVersion}
            </dd>
            <dt>Holds</dt>
            <dd>
              {countsLine(state.preview.counts)} · {formatBytes(state.preview.bytes)}
            </dd>
            <dt>Replaces</dt>
            <dd>{countsLine(state.preview.current)}, here now</dd>
          </dl>
          <span className="lc-settings__note">
            Locust restarts to restore it. What is here now is moved aside in its profile folder, not deleted, so this can
            be undone. Your own model keys are never in a backup, and stay as they are.
          </span>
          <div className="lc-backup__row">
            <button type="button" className="lc-button lc-button--danger" onClick={() => void restore(state.preview.folder)}>
              Restore and restart
            </button>
            <button type="button" className="lc-button" onClick={() => setState({ kind: 'idle' })}>
              Cancel
            </button>
          </div>
        </div>
      )}
      {doing === 'restoring' && <span className="lc-settings__note">Asking for the restore…</span>}
      {state.kind === 'restarting' && <span className="lc-settings__note">Restarting to restore…</span>}
      {state.kind === 'error' && <span className="lc-settings__note lc-tone-red">{state.message}</span>}
    </div>
  )
}

/**
 * The update control.
 *
 * The app checks on its own and downloads on its own; it never installs on
 * its own, because a restart under a running mission would cut the run and
 * leave its record without a terminal receipt. So the last step is a button,
 * and it refuses while anything is running.
 */
function UpdateControl({
  update,
  onCheck,
  onInstall
}: {
  readonly update: AppUpdateState | undefined
  readonly onCheck: () => Promise<AppUpdateResponse>
  readonly onInstall: () => Promise<AppUpdateResponse>
}): ReactElement {
  const [busy, setBusy] = useState(false)
  const [refusal, setRefusal] = useState<string>()
  const [installing, setInstalling] = useState(false)

  const phase = update?.phase ?? 'idle'
  const line =
    phase === 'unsupported'
      // macOS updates only a signed app, and the first Mac builds are not
      // signed yet: said, with the way to the next build.
      ? typeof navigator !== 'undefined' && /Macintosh|Mac OS X/.test(navigator.userAgent)
        ? 'On a Mac this build cannot update itself yet. Download the newest from the releases page and replace it.'
        : 'This build cannot update itself. Installed copies check on their own.'
      : phase === 'checking'
        ? 'Checking…'
        : phase === 'current'
          ? 'Up to date.'
          : phase === 'available'
            // A Linux .deb or .rpm is the package manager's to replace (0.789): the new package is handed over.
            ? update?.downloadUrl !== undefined
              ? `Version ${update.availableVersion ?? ''} is out. Download it, then open the file to install it.`
              : `Version ${update?.availableVersion ?? ''} is available. Downloading it now.`
            : phase === 'downloading'
              ? `Downloading ${update?.availableVersion ?? ''}${update?.percent === undefined ? '' : ` · ${String(update.percent)}%`}`
              : phase === 'ready'
                ? installing
                  ? installingLine(update?.availableVersion)
                  : `Version ${update?.availableVersion ?? ''} is downloaded and ready to install. ${INSTALL_TAKES}`
                : phase === 'failed'
                  ? update?.message ?? 'The update check could not complete.'
                  : 'Not checked yet.'

  return (
    <div className="lc-retention">
      <div className="lc-retention__row">
        <span className={`lc-settings__note${phase === 'failed' ? ' lc-tone-red' : ''}`}>{line}</span>
        <button
          type="button"
          className="lc-button"
          disabled={busy || phase === 'unsupported' || phase === 'checking'}
          // Why it cannot be pressed, on the control itself (the screens sweep, 0.734): the line beside it says it too.
          {...(phase === 'unsupported' ? { title: line } : phase === 'checking' || busy ? { title: 'Checking now.' } : {})}
          onClick={() => {
            setBusy(true)
            setRefusal(undefined)
            void onCheck().finally(() => {
              setBusy(false)
            })
          }}
        >
          Check now
        </button>
        {phase === 'available' && update?.downloadUrl !== undefined && (
          <button
            type="button"
            className="lc-button"
            onClick={() => {
              setRefusal(undefined)
              void window.desktop?.openLink(update.downloadUrl!).then((response) => {
                if (!response.ok) setRefusal(response.message)
              })
            }}
          >
            Download
          </button>
        )}
        {phase === 'ready' && (
          <button
            type="button"
            className="lc-button"
            disabled={busy}
            onClick={() => {
              setBusy(true)
              setRefusal(undefined)
              void onInstall()
                .then((response) => {
                  if (!response.ok) setRefusal(response.error.message)
                  else setInstalling(true)
                })
                .finally(() => {
                  setBusy(false)
                })
            }}
          >
            Install and restart
          </button>
        )}
      </div>
      {refusal !== undefined && <span className="lc-settings__note lc-tone-red">{refusal}</span>}
    </div>
  )
}

/**
 * One muted line: the runtime's own MCP servers and hooks by name, with the
 * files they were read from in the tooltip. Nothing configured says so in
 * words, so an empty line never reads as "not checked".
 */
/**
 * The explanation a section used to open with, folded under one line.
 * Settings read as a manual: every heading had a paragraph before its first
 * control (Colin, 2026-09-05: "the settings screen looks a little clunky").
 * The words are all still here; they wait behind "How it works".
 */
function More({ children }: { readonly children: ReactNode }): ReactElement {
  return (
    <details className="lc-settings__more">
      <summary>How it works</summary>
      <div className="lc-settings__moretext">{children}</div>
    </details>
  )
}

/**
 * "Every connection, listed" (0.618, the PRD's R22): docs/NETWORK.md in the
 * public copy, in the person's browser. If the host will not open it, its own
 * sentence says why, beside the link (a-refused-link-says-so).
 */
function NetworkListLink(): ReactElement {
  const [refused, setRefused] = useState<string>()
  return (
    <>
      <button
        type="button"
        className="lc-linkbutton"
        onClick={() => {
          setRefused(undefined)
          void window.desktop?.openLink(NETWORK_DOC_LINK).then((answer) => setRefused(answer.ok ? undefined : answer.message))
        }}
      >
        Every connection, listed
      </button>
      {refused !== undefined && <span className="lc-link__refusal" role="status"> {refused}</span>}
    </>
  )
}

/** The first few names and a count for the rest, so a long list stays one line. */
function fewNames(names: readonly string[], limit = 5): string {
  return names.length <= limit ? names.join(', ') : `${names.slice(0, limit).join(', ')} +${String(names.length - limit)}`
}

/**
 * The one line a person has to run, with a button that copies it.
 *
 * Shown only for a runtime Locust cannot find. Locust does not run this for
 * them: installing software on someone's machine on their behalf is a
 * different promise from running the agents they already chose, and the app
 * has no business making it without being asked. Copying is one click; the
 * terminal is theirs.
 */
function InstallCommand({ command }: { readonly command: string }): ReactElement {
  const [copied, setCopied] = useState(false)
  const copy = (): void => {
    const done = (): void => {
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1_600)
    }
    // `navigator.clipboard` is the right call and is not always available to a
    // packaged page; the textarea is what works everywhere else.
    navigator.clipboard?.writeText(command).then(done).catch(() => {
      const field = document.createElement('textarea')
      field.value = command
      document.body.appendChild(field)
      field.select()
      try {
        document.execCommand('copy')
        done()
      } finally {
        field.remove()
      }
    })
  }
  return (
    <div className="lc-installline">
      <code className="lc-installline__command lc-mono">{command}</code>
      <button type="button" className="lc-installline__copy" onClick={copy}>
        {copied ? 'Copied' : 'Copy'}
      </button>
    </div>
  )
}

function RuntimeSetupLine({ setup }: { readonly setup: PublicRuntimeSetup }): ReactElement {
  const parts: string[] = []
  if (setup.mcpServers.length > 0) parts.push(`Connectors (MCP): ${setup.mcpServers.join(', ')}`)
  if (setup.hooks.length > 0) parts.push(`Hooks: ${setup.hooks.join(', ')}`)
  if (setup.skills.length > 0) parts.push(`Skills: ${fewNames(setup.skills)}`)
  if (setup.agents.length > 0) parts.push(`Agents: ${fewNames(setup.agents)}`)
  if (setup.unreadable.length > 0) parts.push(`${String(setup.unreadable.length)} config file${setup.unreadable.length === 1 ? '' : 's'} could not be read`)
  const text = parts.length === 0 ? 'No connectors, hooks, skills or agents set up' : parts.join(' · ')
  const title = [...setup.sources.map((path) => `read: ${path}`), ...setup.unreadable.map((path) => `unreadable: ${path}`)].join('\n')
  return (
    <div className="lc-runtimerow__detail lc-runtimerow__setup" title={title.length === 0 ? 'No configuration files found' : title}>
      {text}
    </div>
  )
}

/**
 * The way a person says "this broke".
 *
 * Until 0.138.0 there was none. An uncaught exception raised a dialog that
 * named `locust-errors.log`, which meant the only people who knew the file
 * existed were the ones who had already hit a crash the main process could
 * throw -- and a renderer crash, the one a person would describe as "Locust
 * disappeared", raised no dialog and wrote no line at all.
 *
 * So this exists to be found BEFORE it is needed. It says where the file is,
 * how big it is, and what is in it, because a person deciding whether to
 * send a log to someone is entitled to know what they are sending.
 *
 * Reveals, never opens -- `main/reveal-file.ts` sets that rule out at
 * length. And it takes no path: the host knows the only answer, so there is
 * nothing for the renderer to propose.
 */
function ProblemReport(): ReactElement {
  const [report, setReport] = useState<DiagnosticsReport>()
  const [asking, setAsking] = useState(false)
  useEffect(() => {
    const bridge = window.desktop
    if (bridge === undefined) return
    void bridge.diagnosticsReport().then(setReport).catch(() => undefined)
  }, [])
  return (
    <>
      <dl className="lc-receipt lc-receipt--flush">
        <dt>Log</dt>
        <dd className="lc-mono">{report?.path ?? 'in this profile'}</dd>
        <dt>On disk</dt>
        <dd className="lc-mono">
          {report === undefined ? 'measuring…' : report.exists ? formatBytes(report.byteTotal) : 'nothing yet'}
        </dd>
      </dl>
      <More>
        <p>
          The log records what happened, never what was said. Crashes, a window that stopped answering,
          and the version you were on — no messages, no file contents, and nothing from a conversation. That is
          what makes it safe to send.
        </p>
        <p>It is capped, and rolls over once. Nothing in it leaves this machine unless you send it.</p>
      </More>
      {/*
        * Somewhere to send it (the beta handover, 2026-09-23: nothing said
        * where), in Claude Code's own shape (Colin: "for bug reporting we can
        * use what claude code does"): the Send feedback box. From here it
        * carries no conversation; a conversation's own menu sends that one.
        */}
      <div className="lc-feedbackactions">
        <button type="button" className="lc-button" onClick={() => setAsking(true)}>
          Send feedback
        </button>
        <button
          type="button"
          className="lc-button"
          onClick={() => {
            void window.desktop?.revealDiagnostics()
          }}
        >
          Show the log
        </button>
      </div>
      {asking && <FeedbackDialog onClose={() => setAsking(false)} />}
    </>
  )
}

/**
 * A3.3: the person's check after a teammate's edits, for THIS folder. Set
 * here and only here -- a project never names a command Locust runs.
 *
 * And what happens when it fails (0.812, shared/check-fix.ts): shown with a
 * button, as before, or sent back to the teammate to fix, at most a few times.
 * Off until the person turns it on, because each fix is one more turn.
 */
export function CheckAfterEditsRow({
  saved,
  onSave,
  fixTries,
  onFixTriesChange
}: {
  readonly saved: string
  readonly onSave: (command: string) => void
  readonly fixTries?: number
  readonly onFixTriesChange?: (next: number | undefined) => void
}): ReactElement {
  const [draft, setDraft] = useState(saved)
  useEffect(() => setDraft(saved), [saved])
  const changed = draft.trim() !== saved
  const hasCheck = saved.length > 0
  return (
    <div className="lc-policyrow lc-policyrow--stack">
      <span className="lc-tag">CHECK AFTER EDITS</span>
      <span className="lc-settings__note">
        A command Locust runs in a teammate&apos;s folder after a turn that changed files, such as npm test. Only failures
        that were not there at the last check count. Set here only: a project never chooses its own.
      </span>
      <form
        className="lc-checkrow"
        onSubmit={(event) => {
          event.preventDefault()
          onSave(draft.trim())
        }}
      >
        <input
          className="lc-input lc-mono"
          aria-label="Check after edits"
          value={draft}
          maxLength={500}
          placeholder="npm test"
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => setDraft(event.target.value)}
        />
        <button type="submit" className="lc-button" disabled={!changed}>
          {draft.trim().length === 0 && saved.length > 0 ? 'Turn off' : 'Save'}
        </button>
      </form>
      {onFixTriesChange !== undefined && (
        <div className="lc-checkfix">
          <div className="lc-checkfix__choice">
            <span className="lc-settings__note" id="lc-checkfix-label">When it fails</span>
            <div className="lc-segmented" role="radiogroup" aria-labelledby="lc-checkfix-label">
              <button
                type="button"
                role="radio"
                aria-checked={fixTries === undefined}
                className={`lc-button${fixTries === undefined ? ' is-active' : ''}`}
                onClick={() => onFixTriesChange(undefined)}
              >
                Show me
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={fixTries !== undefined}
                disabled={!hasCheck && fixTries === undefined}
                title={!hasCheck && fixTries === undefined ? 'Save a check first.' : undefined}
                className={`lc-button${fixTries !== undefined ? ' is-active' : ''}`}
                onClick={() => onFixTriesChange(fixTries ?? DEFAULT_CHECK_FIXES)}
              >
                Send it back to fix
              </button>
            </div>
            {fixTries !== undefined && (
              <>
                <span className="lc-settings__note" id="lc-checkfix-most">at most</span>
                <div className="lc-segmented is-numeric" role="radiogroup" aria-labelledby="lc-checkfix-most">
                  {Array.from({ length: MAX_CHECK_FIXES }, (_, index) => index + 1).map((most) => (
                    <button
                      key={most}
                      type="button"
                      role="radio"
                      aria-checked={fixTries === most}
                      aria-label={`${String(most)} ${most === 1 ? 'fix' : 'fixes'}`}
                      className={`lc-button${fixTries === most ? ' is-active' : ''}`}
                      onClick={() => onFixTriesChange(most)}
                    >
                      {String(most)}
                    </button>
                  ))}
                </div>
                <span className="lc-settings__note">{fixTries === 1 ? 'fix' : 'fixes'}</span>
              </>
            )}
          </div>
          <span className="lc-settings__note">
            {fixTries === undefined
              ? hasCheck
                ? 'What is new in the failure is shown under the turn, with a button to send it to the teammate.'
                : 'Save a check first. Then a failure can be shown to you, or sent back to the teammate to fix.'
              : `After a turn you sent, what is new in the failure goes back to the same teammate, on the same model, until the check passes: ${String(fixTries)} time${fixTries === 1 ? '' : 's'} at most, and it stops sooner when a fix changes nothing. Each fix is one more turn.`}
          </span>
        </div>
      )}
    </div>
  )
}

/** Three bots wearing the Terminal faces choice beside it: at work, in thought, done. */
/** The bots the Plush row shows, in whichever material is chosen. */
const PLUSH_PREVIEW: readonly ('cat' | 'cloud' | 'clover')[] = ['cat', 'cloud', 'clover']

const TERMINAL_FACES_PREVIEW: readonly (readonly ['droid' | 'ghost' | 'cat', EyeGlyphs])[] = [
  ['droid', ['>', '▮']],
  ['ghost', ['•', '•']],
  ['cat', ['^', '^']]
]

export function SettingsScreen({
  scout,
  scoutTeam,
  scoutModel,
  scoutRoute,
  onChangeScoutModel,
  runtimes,
  limitedRuntimes,
  usageWindows,
  setupNotes,
  runtimeUpdates,
  onKeepAgentsCurrent,
  onUpdateAgent,
  onInstall,
  installing,
  installLine,
  installFailure,
  npmMissing,
  runtimeSetup,
  cliArtifacts,
  workspaceBrief,
  worktrees,
  onRemoveWorktree,
  workspacePath,
  workspaceMade = false,
  onChooseFolder,
  ledgerPath,
  build,
  storage,
  update,
  onCheckUpdate,
  onInstallUpdate,
  onUpdateLane,
  loginItem = { openAtLogin: false, available: false },
  onLoginItemChange,
  onGeneralOpen,
  keepRunning = false,
  onKeepRunningChange,
  stayAwake = false,
  onStayAwakeChange,
  relay,
  onRelayChange,
  interrupt,
  onInterruptChange,
  autoMode,
  planRunsOut,
  carryOnChoices = [],
  onPlanRunsOutChange,
  askConnectors,
  keepATodoList,
  onKeepATodoListChange,
  claudeOwnSkills = false,
  onClaudeOwnSkillsChange,
  onAskConnectorsChange,
  onOwnModelsChanged,
  swarm,
  tube,
  replySize,
  checkCommand,
  onCheckCommandSave,
  checkFixTries,
  onCheckFixTriesChange,
  metal,
  metalStrength,
  metalMotion,
  metalBend,
  onMetalChange,
  terminalFaces = true,
  onTerminalFacesChange,
  plush = false,
  onPlushChange,
  onReplySizeChange,
  onTubeChange,
  onSwarmChange,
  onAutoModeChange,
  relayHopCap,
  onRelayHopCapChange,
  memoryMode,
  layout,
  layoutMode,
  onLayoutChange,
  onMemoryModeChange,
  memoryCount,
  memoryWaiting,
  onOpenMemory,
  onPreviewPrune,
  onListTrash,
  onRestoreMission,
  onEmptyTrash,
  changelog,
  initialPage,
  onPrune,
  onPickProfileFolder,
  onBackUpProfile,
  onPreviewProfileRestore,
  onRestoreProfile
}: {
  /** Scout (0.756): what it holds, the model it reads with, and the free model a new one would read with. */
  readonly scout?: ScoutState
  /** Who may prepare Scout's fixes (0.828): the team but Scout. */
  readonly scoutTeam?: readonly { readonly teammateId: string; readonly name: string }[]
  readonly scoutModel?: string
  readonly scoutRoute?: { readonly runtime: string; readonly model: string }
  readonly onChangeScoutModel?: () => void
  readonly runtimes: readonly PublicRuntimeStatus[]
  /** Runtimes whose last run ended on the account's usage limit, with its own words. */
  readonly limitedRuntimes: ReadonlyMap<string, string>
  /** The latest still-allowed rate-limit reading per runtime, in words. */
  readonly usageWindows?: ReadonlyMap<string, string>
  /** What each CLI said about its own setup on its newest run (missionView's setupNotesOf). */
  readonly setupNotes?: ReadonlyMap<string, readonly string[]>
  /** What keeping the coding agents current has done (runtime-updates.ts); undefined until read. */
  readonly runtimeUpdates?: RuntimeUpdatesState
  /** Update the agents without being asked, or not. */
  readonly onKeepAgentsCurrent?: (automatic: boolean) => void
  /** Update pressed on a runtime's row. */
  readonly onUpdateAgent?: (runtime: string) => void
  /**
   * INSTALL FROM HERE TOO (0.780). The row said "Press Install" and only Home's list had the button; on a machine
   * with no Node the npm line under it fails in the person's terminal, so the one way in was on another screen
   * (clean-machine drive, packaged 0.779). Home's own install: one at a time, its line and its failure the same.
   */
  readonly onInstall?: (runtime: string) => void
  /** The runtime being installed now, from Home or here. */
  readonly installing?: string
  /** The install's live line ("12s · npm's last line"), while it runs. */
  readonly installLine?: string
  /** Why the last install did not finish, and what to do. */
  readonly installFailure?: { readonly what: string; readonly next: string }
  /** No npm at all, the bundled one included: the button is off and says why. */
  readonly npmMissing?: boolean
  /** Each runtime's own MCP servers and hooks, by runtime id; undefined until read. */
  readonly runtimeSetup: Readonly<Record<string, PublicRuntimeSetup>> | undefined
  /**
   * What the person configured inside the CLIs themselves, shown under the
   * runtime each belongs to. Locust neither made nor runs these.
   */
  readonly cliArtifacts?: readonly PublicRuntimeArtifact[]
  /** The folder's LOCUST.md as last read: null when none, undefined until the host has answered. */
  readonly workspaceBrief: PublicWorkspaceBrief | null | undefined
  /** The teammates' own worktrees under the folder, and why there can be none; undefined until read. */
  readonly worktrees: { readonly list: readonly PublicWorktree[]; readonly reason: string | undefined } | undefined
  readonly onRemoveWorktree: (teammateId: string, discard?: readonly string[]) => Promise<WorktreeRemoval | undefined>
  /** The folder every teammate works in; undefined when none is chosen. */
  readonly workspacePath: string | undefined
  readonly workspaceMade?: boolean
  readonly onChooseFolder: () => void
  readonly ledgerPath: string | undefined
  /** Which build this is; undefined until the host has answered. */
  readonly build: { readonly version: string; readonly packaged: boolean } | undefined
  /** What the local history costs; undefined until the host has answered. */
  readonly storage: PublicStorageReport | undefined
  /** Where an update stands; undefined until the host has said anything. */
  readonly update: AppUpdateState | undefined
  readonly onCheckUpdate: () => Promise<AppUpdateResponse>
  readonly onInstallUpdate: () => Promise<AppUpdateResponse>
  /** Every build, or the one a day testers get (main/update-lane.ts). */
  readonly onUpdateLane?: (everyBuild: boolean) => void
  /** What Windows holds for sign-in. Read again each time this page opens. */
  readonly loginItem?: LoginItemState
  readonly onLoginItemChange?: (openAtLogin: boolean) => void
  /** Settings opened on General: read the login item again, so the switch shows what Windows holds. */
  readonly onGeneralOpen?: () => void
  /** Keep Locust running when the window is closed. Off unless the person turned it on. */
  readonly keepRunning?: boolean
  readonly onKeepRunningChange?: (keepRunning: boolean) => void
  /** Keep the computer awake while it is plugged in, so routines and Scout run on time (0.838). */
  readonly stayAwake?: boolean
  readonly onStayAwakeChange?: (stayAwake: boolean) => void
  /** Whether teammates start runs to answer each other. */
  readonly relay: boolean
  readonly onRelayChange: (relay: boolean) => void
  /** Whether a teammate may stop another's run to be heard now. */
  readonly interrupt: boolean
  readonly onInterruptChange: (interrupt: boolean) => void
  /** Whether the Auto permission mode may be chosen at all. */
  readonly autoMode: boolean
  /** Workspace-wide: every mission at its model's maximum effort. */
  readonly swarm: boolean
  readonly onSwarmChange: (swarm: boolean) => void
  readonly tube: TubePreference
  readonly onTubeChange: (tube: TubePreference) => void
  /** How big a reply is set. The person's, not the app's -- see ReplyTextSize. */
  readonly replySize: ReplyTextSize
  /** A3.3: this folder's check after edits; empty for none. */
  readonly checkCommand?: string
  readonly onCheckCommandSave?: (command: string) => void
  /** 0.812: this folder's most fixes when the check fails; undefined shows the failure instead. */
  readonly checkFixTries?: number
  readonly onCheckFixTriesChange?: (next: number | undefined) => void
  /** The send button's metal; every option the design pass offered. */
  readonly metal: MetalPreset
  readonly metalStrength: MetalStrength
  readonly metalMotion: MetalMotion
  readonly metalBend: boolean
  readonly onMetalChange: (next: {
    readonly metal?: MetalPreset
    readonly metalStrength?: MetalStrength
    readonly metalMotion?: MetalMotion
    readonly metalBend?: boolean
  }) => void
  /** Every bot wears a screen for a face (0.561). */
  readonly terminalFaces?: boolean
  readonly onTerminalFacesChange?: (on: boolean) => void
  /** Our bots in plush faux fur instead of plastic (0.577). */
  readonly plush?: boolean
  readonly onPlushChange?: (on: boolean) => void
  readonly onReplySizeChange: (size: ReplyTextSize) => void
  readonly onAutoModeChange: (autoMode: boolean) => void
  /** Who carries the work on when a plan runs out (0.774). Absent: the card offers the first agent that can. */
  readonly planRunsOut?: PlanRunsOut
  /** The agents that can carry work on here, by name. */
  readonly carryOnChoices?: readonly { readonly runtime: MissionRuntimeId; readonly label: string }[]
  /** null clears the pick. */
  readonly onPlanRunsOutChange?: (next: PlanRunsOut | null) => void
  readonly askConnectors: boolean
  readonly onAskConnectorsChange: (askConnectors: boolean) => void
  /** A model of the person's own was added or removed: the model list is read again. */
  readonly onOwnModelsChanged?: () => void
  readonly keepATodoList: boolean
  readonly onKeepATodoListChange: (keepATodoList: boolean) => void
  /** A Claude Code teammate may use the person's own skills too (0.679). */
  readonly claudeOwnSkills?: boolean
  readonly onClaudeOwnSkillsChange?: (claudeOwnSkills: boolean) => void
  /** The autonomy budget: automatic replies one exchange may use before it waits for a person. */
  readonly relayHopCap: number
  readonly onRelayHopCapChange: (cap: number) => void
  /** What happens to a memory a teammate writes. */
  readonly memoryMode: MemoryMode
  /** What the person chose; `auto` follows the window width. */
  readonly layout: LayoutPreference
  /** What that actually resolves to right now, so `auto` can say which. */
  readonly layoutMode: 'compact' | 'wide'
  readonly onLayoutChange: (layout: LayoutPreference) => void
  readonly onMemoryModeChange: (mode: MemoryMode) => void
  readonly memoryCount: number
  /** Memories a teammate proposed that wait for a keep or a forget. */
  readonly memoryWaiting: number
  readonly onOpenMemory: () => void
  readonly onPreviewPrune: (days: number) => Promise<MissionPruneResponse>
  readonly onListTrash: () => Promise<TrashListResponse>
  readonly onRestoreMission: (missionId: string) => Promise<TrashMutationResponse>
  readonly onEmptyTrash: () => Promise<TrashMutationResponse>
  /** What changed in the running build; undefined until the host has answered. */
  readonly changelog: AppChangelog | undefined
  /** The page Settings opens on: What's new, from the splash's "See every version". */
  readonly initialPage?: SettingsPageId
  readonly onPrune: (days: number) => Promise<MissionPruneResponse>
  /** Back up and restore (0.614): the folder picker, the backup, a restore's preview, the restore. */
  readonly onPickProfileFolder: (purpose: 'backup' | 'restore') => Promise<string | undefined>
  readonly onBackUpProfile: (folder: string) => Promise<ProfileBackupResponse>
  readonly onPreviewProfileRestore: (folder: string) => Promise<ProfileRestorePreview>
  readonly onRestoreProfile: (folder: string) => Promise<ProfileRestoreResponse>
}): ReactElement {
  const [page, setPage] = useState<SettingsPageId>(initialPage ?? 'app')
  const policy = useOrganizationPolicy()
  const autoManaged = !policyModeAllowed(policy, 'auto')
  const [query, setQuery] = useState('')
  // Bumped when a connector was added or taken back, so the list above reads again (0.716).
  const [connectorsRead, setConnectorsRead] = useState(0)
  const asked = query.trim()
  // A page earns its place in the list when its name or one of its settings
  // matches. With no search every page is there, which is the ordinary state.
  const shownPages =
    asked.length === 0
      ? SETTINGS_PAGES
      : SETTINGS_PAGES.filter((entry) => pageMatches(entry, asked))
  // A search that hides the page you were on moves you to the first that
  // survived, rather than showing an empty pane beside a list of matches.
  const shownPage = shownPages.some((entry) => entry.id === page) ? page : shownPages[0]?.id
  useEffect(() => {
    if (shownPage === 'app') onGeneralOpen?.()
    // The page opening is the event. The callback is recreated every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shownPage])
  /*
   * AND THE PANE OPENS AT WHAT WAS SEARCHED FOR, not at the top of the page.
   *
   * Grok, pass 9: typing "trash" switched to This app and left the pane on
   * Updates, with Trash further down the same page. "I think switching the
   * page is right. I think not scrolling to the heading is the miss." Five
   * pages are short enough to survive that; the words a person types that
   * are NOT headings are not -- someone who types "memory" and lands at the
   * top of How teammates work has no way to tell which of four sections
   * answered them.
   */
  const pane = useRef<HTMLDivElement>(null)
  const landOn = shownPage === undefined || asked.length === 0
    ? undefined
    : matchedHeadings(SETTINGS_PAGES.find((entry) => entry.id === shownPage)!, asked)[0]
  useEffect(() => {
    if (landOn === undefined) return
    const headings = pane.current?.querySelectorAll('.lc-settings__heading') ?? []
    for (const heading of headings) {
      if (heading.textContent?.trim() !== landOn) continue
      // The section, so the heading does not land flush against the top
      // edge with its own words half under the search box.
      ;(heading.closest('.lc-settings__section') ?? heading).scrollIntoView({ block: 'start' })
      return
    }
    // Nothing to scroll to is the ordinary case for a page-name match, and
    // the top of the page is already the right place for it.
  }, [landOn, shownPage])

  return (
    <div className="lc-screen">
      <ScreenHeader
        title="Settings"
        meta={
          build === undefined
            ? 'workspace · local only'
            : `Locust ${build.version}${build.packaged ? '' : ' · development build'} · local only`
        }
      />
      <div className="lc-settings">
        <nav className="lc-settings__nav" aria-label="Settings sections">
          <input
            type="search"
            className="lc-settings__search"
            placeholder="Search settings"
            value={query}
            aria-label="Search settings"
            onChange={(event) => setQuery(event.target.value)}
          />
          {shownPages.length === 0 ? (
            <p className="lc-settings__note">Nothing matches. The pages are still here; clear the search to see them.</p>
          ) : (
            /*
             * Pages under their group's label, each with its icon: the list
             * Claude's own settings draw (0.393). A group label is said once,
             * above the first page of its run, so a search that leaves two
             * pages of one group still names the group they are in.
             */
            shownPages.map((entry, index) => (
              <Fragment key={entry.id}>
              {shownPages[index - 1]?.group !== entry.group && <span className="lc-settings__navgroup">{entry.group}</span>}
              <button
                type="button"
                className={`lc-settings__navitem${entry.id === shownPage ? ' is-current' : ''}`}
                aria-current={entry.id === shownPage ? 'page' : undefined}
                onClick={() => setPage(entry.id)}
              >
                <span className="lc-settings__navlabel">
                  <span className="lc-settings__navicon" aria-hidden="true"><Icon name={entry.icon} size={16} /></span>
                  {entry.label}
                </span>
                {query.trim().length > 0 && (
                  <span className="lc-settings__navhits">
                    {/*
                      * The heading, not the word typed. Someone who types
                      * "memory" is told "What your team remembers", which is
                      * what they will be reading for once the page opens --
                      * echoing their own word back would say nothing.
                      */}
                    {matchedHeadings(entry, asked).join(' · ')}
                  </span>
                )}
              </button>
              </Fragment>
            ))
          )}
        </nav>
        <div className="lc-settings__pane" ref={pane}>
        {/*
          * A PAGE THAT OPENS ON SWITCHES OPENS WITH ITS NAME (0.743). Colin, 2026-10-11: "the actual contents of the
          * settings menu is kind of trashy compared to the rest of the app". On General, Teammates and Appearance
          * the first settings were rows of 13px names, and the first 16px heading came further down -- Startup,
          * Skills from GitHub, Send button -- so a section mid-page outranked everything above it. Claude's own
          * settings open each group with its name at that size. The other pages already open on a heading.
          */}
        {(shownPage === 'app' || shownPage === 'teammates' || shownPage === 'appearance') && (
          <p className="lc-settings__pagehead">{SETTINGS_PAGES.find((entry) => entry.id === shownPage)?.label}</p>
        )}
        {/*
          * Where the teammates work. A workspace-wide fact, so it sits with
          * the other workspace-wide settings rather than on the intro screen
          * (Colin, 2026-09-05). The composer carries a chip for the same
          * thing, the way Claude Code states its own folder on the bar.
          */}
        {/*
          * GROUPED BY WHAT A SETTING IS ABOUT, and ordered by how often it
          * is touched.
          *
          * Colin, 2026-09-14: "our settings page is a disaster, just copy
          * how claude code does and organize theirs". It was thirteen
          * subjects in one unbroken scroll, in the order they happened to be
          * built: Updates third, the two appearance settings at opposite
          * ends, and one section stacking the boot screen, Swarm and Auto
          * mode together because they were added on the same day.
          *
          * Five areas now, each named for what is in it. The prose stays
          * where it was -- behind `More` -- because the problem was never
          * that a setting explained itself, it was that you could not find
          * the setting.
          */}
        {shownPage === 'chat' && (
          <section className="lc-settings__section">
            <h2 className="lc-settings__heading">Your commands</h2>
            <p className="lc-settings__lede">
              Markdown files become slash commands in the chat box, next to the agents&apos; own. Yours are on this
              computer. A folder&apos;s are in that folder, and only there.
            </p>
            <ChatCommands />
          </section>
        )}
        {shownPage === 'chat' && (
          <section className="lc-settings__section">
            <h2 className="lc-settings__heading">How to write one</h2>
            <p className="lc-settings__note">
              The top of the file can name a description, a teammate, and a mode. The rest is the prompt. $ARGUMENTS is
              everything typed after the name. $1 and $2 are the words, and the last one takes the rest. With none of
              those, what you type is added after a blank line. A line with !`command` inserts that command&apos;s output
              when you choose it. One in this folder asks before it runs; one of yours runs at once. Choosing a command
              fills the box. Send is what sends it.
            </p>
          </section>
        )}
        {shownPage === 'workspace' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Project folder</h2>
          {/*
            * Said as what moves with the folder and what does not. "Every
            * teammate works inside one folder" read as teammates being tied to
            * it -- Colin, 2026-09-26, holding this screen: "does this mean if
            * the user switches workspace the teammates wont behave correctly?"
            * They do: the roster, routes and modes are the app's; the folder's
            * are its files, conversations, memories and LOCUST.md.
            */}
          <p className="lc-settings__lede">
            Your teammates are the same in every folder. The folder is what they work on: its files, its conversations,
            what they remember about it, and its LOCUST.md. Anything remembered for everywhere goes with them.
          </p>
          <More>
            <p>Changing the folder reopens Locust, so stop anything running first.</p>
            <p>
              A notes file named LOCUST.md at the folder&rsquo;s root is given to every teammate, on every AI agent, before each
              run. A teammate with Own copy on works in its own copy of the folder (a git worktree of its repository),
              kept under .locust/worktrees. Each turn it finishes is saved on its branch as a commit; Review
              changes in its conversation shows them and lands them on your branch as one commit of yours.
              Removing one here keeps its branch.
            </p>
          </More>
          <div className="lc-settingcard">
          <div className={`lc-folder${workspacePath === undefined ? ' is-missing' : ''}`}>
            <div className="lc-folder__text">
              <div className="lc-folder__label">
                {workspacePath === undefined ? 'No folder chosen' : workspaceMade ? 'Teammates work in a folder Locust made' : 'Teammates work in'}
              </div>
              <div className={`lc-folder__path${workspacePath === undefined ? '' : ' lc-mono'}`}>
                {workspacePath ?? 'Pick a project folder before the first mission.'}
              </div>
            </div>
            <button
              type="button"
              className="lc-button"
              onClick={onChooseFolder}
              title={workspacePath === undefined ? 'Choose the folder your teammates work in' : 'Choose another folder. Locust reopens in it.'}
            >
              {workspacePath === undefined ? 'Choose folder' : 'Change'}
            </button>
          </div>
          {/*
            * Said in front of the button, not behind a fold.
            *
            * Every service binds its folder at start-up, so changing it
            * reopens the app -- which is the honest switch and looks exactly
            * like a crash if nobody was told. Colin has reported it twice as
            * one: "changing worktree folder still crashes/resets the app."
            * The sentence existed; it was inside `More`, which is closed.
            */}
          {workspacePath !== undefined && (
            <p className="lc-settings__note lc-folder__warns">
              Locust reopens in the folder you choose. Stop anything running first.
            </p>
          )}
          {workspacePath !== undefined && workspaceBrief !== undefined && (
            <div className="lc-policyrow">
              <span className="lc-tag">LOCUST.md</span>
              <span className="lc-settings__note">
                {workspaceBrief === null
                  ? 'None in this folder. Add a notes file named LOCUST.md at its root and every teammate, on every AI agent, is given it before each run.'
                  : `${String(workspaceBrief.lines)} line${workspaceBrief.lines === 1 ? '' : 's'} briefed to every teammate before each run${workspaceBrief.truncated ? ' — longer than 200 lines, so the rest is not loaded. Put long checklists in files of their own and name them in LOCUST.md; a teammate reads one when it applies' : ''}.`}
              </span>
            </div>
          )}
          {workspacePath !== undefined && onCheckCommandSave !== undefined && (
            <CheckAfterEditsRow
              saved={checkCommand ?? ''}
              onSave={onCheckCommandSave}
              {...(checkFixTries === undefined ? {} : { fixTries: checkFixTries })}
              {...(onCheckFixTriesChange === undefined ? {} : { onFixTriesChange: onCheckFixTriesChange })}
            />
          )}
          {workspacePath !== undefined && worktrees !== undefined && (
            <div className="lc-policyrow lc-policyrow--stack">
              <span className="lc-tag">OWN COPIES</span>
              <span className="lc-settings__note">
                {worktrees.reason !== undefined
                  ? `${worktrees.reason} A teammate with Own copy on cannot start until this is fixed.`
                  : worktrees.list.length === 0
                    ? 'None yet. Turn Own copy on in a teammate\'s card and its next run makes one.'
                    : 'Removing one keeps its branch, with what was committed on it; uncommitted changes are named before anything is deleted.'}
              </span>
              {worktrees.list.length > 0 && (
                <div className="lc-worktreelist">
                  {worktrees.list.map((tree) => (
                    <WorktreeRow key={tree.teammateId} tree={tree} onRemove={onRemoveWorktree} />
                  ))}
                </div>
              )}
            </div>
          )}
          </div>
        </section>
        )}
        {shownPage === 'relay' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Between teammates</h2>
          <p className="lc-settings__lede">
            {/*
              * Says what is TRUE NOW. It stated the relay-on behaviour whatever
              * the switch said, so with relay off the lede promised automatic
              * replies while the row under it said messages wait (design
              * pass, 2026-09-22).
              */}
            {relay
              ? `Teammates answer each other on their own, each on its own route, and stop after ${String(relayHopCap)} automatic ${relayHopCap === 1 ? 'reply' : 'replies'}.`
              : 'Teammates do not answer each other on their own. A message waits until its recipient next runs.'}
          </p>
          <More>
            <p>
              When a teammate writes to another, the other can answer on their own: Locust starts a run for
              them with the message as its brief, and their answer starts the sender&rsquo;s next turn, so it
              lands in the thread that asked. They keep going while a reply helps finish the work, and stop
              when one has nothing more to say, or when the budget below is spent.
            </p>
            <p>
              Each teammate answers on their own route — their runtime, model and mode, not the sender&rsquo;s --
              which is how two models end up on one piece of work. Switch replies off to make messages wait
              for you instead.
            </p>
            <p>
              A teammate does one thing at a time, so a message that arrives while they are working waits
              until that run ends. A sender can mark a message urgent when waiting would make it useless --
              &ldquo;stop, I am editing that file&rdquo; is the case it exists for. Letting that stop the
              recipient part-way throws away whatever they had in flight, so it is off until you turn it on;
              their unfinished work stays in their own conversation either way.
            </p>
          </More>
          <div className="lc-settingrows">
            <div className="lc-settingrow">
              <span className="lc-settings__note">
                {/* Named, then what is true now (0.419): the row said only the
                    state, so it never said what the switch was, and its switch
                    was "Switch this on" to a screen reader (fresh-eyes area 13). */}
                {relay ? 'Automatic replies: on. Teammates reply to each other until the work is done.' : 'Automatic replies: off. Messages wait for the recipient\'s next run.'}
              </span>
              <button
                type="button"
                className={`lc-switch${relay ? ' is-on' : ''}`}
                role="switch"
                aria-checked={relay}
                aria-label="Automatic replies"
                onClick={() => onRelayChange(!relay)}
              >
                <span className="lc-switch__knob" />
              </button>
            </div>
            {/*
              * Waiting their turn, or not.
              *
              * A teammate runs one mission at a time, so a message that
              * arrives mid-run waits for that run to end. Usually right, and
              * sometimes far too late: the message worth interrupting for is
              * "stop, I am editing that file", and delivering it once the
              * conflicting work is finished delivers it after the damage.
              *
              * Off by default and described as what it costs, because it
              * DISCARDS: the recipient's turn stops where it stands. Only a
              * sender that asked -- `when="now"` on its message -- can spend
              * it, and the message still goes through the ordinary waiting
              * path afterwards, so nothing here widens how far teammates may
              * go on their own.
              */}
            <div className="lc-settingrow">
              <span className="lc-settings__note">
                {interrupt
                  ? 'Urgent messages interrupt: on. An urgent message stops the recipient part-way. Their unfinished work stays in their own conversation.'
                  : 'Urgent messages interrupt: off. An urgent message still waits for the recipient to finish.'}
              </span>
              <button
                type="button"
                className={`lc-switch${interrupt ? ' is-on' : ''}`}
                role="switch"
                aria-checked={interrupt}
                aria-label="Urgent messages interrupt"
                disabled={!relay}
                // A control that is off for a reason says the reason (Yurt's
                // beta report, #13: "disabled with no reason given").
                title={relay ? undefined : RELAY_FIRST}
                onClick={() => onInterruptChange(!interrupt)}
              >
                <span className="lc-switch__knob" />
              </button>
            </div>
            {/*
              * The autonomy budget. Six was a constant nobody could see or
              * change; the 0.21.2 QA pass (rec. 6) asked for a budget the
              * person owns. Fixed steps rather than a free number: each is
              * a real answer to "how far may they go without me".
              */}
            <div className="lc-settingrow">
              <span className="lc-settings__note">
                Automatic replies per exchange before they wait for you
              </span>
              <div className="lc-segmented is-numeric" role="radiogroup" aria-label="Automatic replies per exchange">
                {[2, 4, 8, 12, 16, 24].map((cap) => (
                  <button
                    key={cap}
                    type="button"
                    role="radio"
                    aria-checked={relayHopCap === cap}
                    className={`lc-button${relayHopCap === cap ? ' is-active' : ''}`}
                    disabled={!relay}
                    title={relay ? undefined : RELAY_FIRST}
                    onClick={() => onRelayHopCapChange(cap)}
                  >
                    {String(cap)}
                  </button>
                ))}
              </div>
            </div>
          </div>
        </section>
        )}

        {shownPage === 'runtimes' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">AI agents &amp; accounts</h2>
          {/*
            * Claude Code's own switch, only where Claude Code is (0.780). On a machine with only OpenCode it was the
            * first thing on this page and could only fail (clean-machine drive, packaged 0.779).
            */}
          {runtimes.some((runtime) => runtime.id === 'claude' && runtime.installed) && <RemoteControlSetting />}
          <p className="lc-settings__lede">
            Each AI agent starts with this PC's sign-in. Claude Code and Codex can also use accounts you add here.
          </p>
          {/*
            Only when NOTHING is installed, which is what someone who has just
            installed Locust and nothing else is looking at. The list is
            ordered for people who already have these; a fresh machine reads
            "Codex CLI" first and is told it needs a ChatGPT account, which
            makes the app look like it costs money to open. One of them needs
            no account at all, and that is the sentence that belongs here.
          */}
          {runtimes.length > 0 && !runtimes.some((runtime) => runtime.installed) && (
            <p className="lc-settings__lede lc-settings__lede--start">
              None of these are on this machine yet. The quickest start is <b>OpenCode</b>: press its Install, no
              account, and its free model runs as soon as it is there. The others below are worth adding
              when you want a stronger model, and each needs its own sign-in.
            </p>
          )}
          <More>
            <p>
              Locust never pools subscriptions or proxies your requests. Under each AI agent is what it has
              set up for itself — connectors (MCP servers) and hooks, read from its own files — so a tool a teammate
              reaches for, or a script that runs mid-run, is never a surprise. Locust changes nothing there.
            </p>
          </More>
          <div className="lc-runtimelist lc-settingcard">
            {/*
              * On a machine with nothing installed the lede says the quickest
              * start is OpenCode and the list under it opened with Codex,
              * Claude, Cursor, Copilot -- OpenCode fifth (QA, 2026-09-06). The
              * sentence and the list disagreed about where to look. Once
              * anything IS installed the existing order stands: connected
              * first, which is what a person with a working setup wants.
              */}
            {(runtimes.some((runtime) => runtime.installed)
              ? // What you can use, first. Frame pass, 2026-09-15: two
                // runtimes that do not exist yet sat in the middle of six
                // that do, so the list had to be read tag by tag.
                runtimeListOrder(runtimes, (runtime) =>
                  routeRowStatus(runtime, integrationOf(runtime.id), false, limitedRuntimes.get(runtime.id)).tag
                )
              : [...runtimes].sort((left, right) =>
                  left.id === FREE_START_RUNTIME ? -1 : right.id === FREE_START_RUNTIME ? 1 : 0
                )
            ).map((runtime) => {
              const status = routeRowStatus(runtime, integrationOf(runtime.id), false, limitedRuntimes.get(runtime.id))
              return (
                <div className="lc-runtimerow" key={runtime.id}>
                  <div className="lc-runtimerow__text">
                    <div className="lc-runtimerow__name">
                      {runtime.displayName}
                      {runtime.version !== null && <span className="lc-runtimerow__version lc-mono">{runtime.version}</span>}
                    </div>
                    {/* "Signed in on this machine" is said once, in the lede; a row only speaks when its state is not the ordinary one. */}
                    {status.tag !== 'READY' && <div className="lc-runtimerow__detail">{status.detail}</div>}
                    {/* Kept current (runtime-updates.ts): a newer version, an update under way, one that landed. */}
                    {updateLine(runtimeUpdates?.agents.find((agent) => agent.runtime === runtime.id)) !== undefined && (
                      <div className="lc-runtimerow__detail lc-runtimerow__update">
                        {updateLine(runtimeUpdates?.agents.find((agent) => agent.runtime === runtime.id))}
                      </div>
                    )}
                    {usageWindows?.get(runtime.id) !== undefined && (
                      // A meter a window, toned only where it is pressing (design pass, 0.715).
                      <div className="lc-runtimerow__detail lc-runtimerow__usage">
                        <UsageMeters said={usageWindows.get(runtime.id)!} />
                      </div>
                    )}
                    {/*
                      * What the CLI said about its own setup on its last run
                      * -- a key its config has that it does not know -- kept
                      * out of the conversations (0.308) and said here, once.
                      */}
                    {(setupNotes?.get(runtime.id) ?? []).slice(0, 3).map((note) => (
                      <div key={note} className="lc-runtimerow__detail lc-runtimerow__setupnote">
                        {runtime.displayName} says: {note}
                      </div>
                    ))}
                    {/* What an Agent Client Protocol agent told Locust it can do, at its last run (W12). */}
                    {runtime.agentCapabilities !== undefined && (
                      <div className="lc-runtimerow__detail lc-runtimerow__abilities">
                        <span>{agentCapabilityHeading(runtime.displayName, runtime.agentCapabilities)}</span>
                        <ul>
                          {agentCapabilityLines(runtime.agentCapabilities).map((line) => (
                            <li key={line}>{line}</li>
                          ))}
                        </ul>
                      </div>
                    )}
                    {/*
                      * The npm line only where the row's Install cannot run it (0.840): a person on a machine with
                      * nothing installed was shown a terminal command, with Copy, beside a button that does it.
                      */}
                    {status.tag === 'NOT INSTALLED' && installCommand(runtime.id) !== undefined && (onInstall === undefined || npmMissing === true) && (
                      <InstallCommand command={installCommand(runtime.id)!} />
                    )}
                    {installing === runtime.id && installLine !== undefined && (
                      <div className="lc-runtimerow__detail lc-mono">{installLine}</div>
                    )}
                    {runtimeSetup?.[runtime.id] !== undefined && <RuntimeSetupLine setup={runtimeSetup[runtime.id]!} />}
                    {runtime.installed && <AgentAccountsSettings runtime={runtime.id} usage={usageWindows} />}
                    {/*
                      * What the person set up inside THIS CLI. It used to sit
                      * on the Automations screen under a heading that
                      * otherwise meant "things you can run", which is what
                      * made its inertness read as brokenness. An agent file
                      * in Codex's config directory is a fact about Codex, and
                      * under Codex it needs no apology.
                      */}
                    <CliArtifacts artifacts={(cliArtifacts ?? []).filter((entry) => entry.runtime === runtime.id)} />
                  </div>
                  <span
                    className={`lc-tag${
                      // Green is available, lime is happening now (design
                      // pass, objection 5). The first-run panel took the
                      // rule in 0.21.2; Settings kept five lime tags for a
                      // list where nothing is running -- seen in the user
                      // session, 2026-09-05.
                      status.tag === 'READY' || status.tag === 'ACTIVE'
                        ? ' is-green'
                        : status.tag === 'SIGN IN'
                          ? ' is-red'
                          : status.tag === 'PREVIEW' || status.tag === 'EXPERIMENTAL' || status.tag === 'AT LIMIT'
                            ? ' is-amber'
                            : ''
                    }`}
                  >
                    {status.tag}
                  </span>
                  {/*
                    * A red tag that does not say what to do is half a message.
                    * The first-run panel has printed this command since it was
                    * built; Settings -- the screen a person opens when a
                    * runtime is wrong -- drew the same red tag and stopped
                    * there (Grok's audit, 2026-09-13).
                    */}
                  {status.tag === 'SIGN IN' && <SignInButton runtime={runtime.id} />}
                  {/* Another account, or the same one afresh (Colin, 2026-10-02: "change accounts in settings"). */}
                  {(status.tag === 'READY' || status.tag === 'ACTIVE') && runtime.auth === 'authenticated' && <SignInButton runtime={runtime.id} again />}
                  {/* A newer version is out: the download is the person's to start (0.303). */}
                  {offersUpdate(runtimeUpdates?.agents.find((agent) => agent.runtime === runtime.id)) && onUpdateAgent !== undefined && (
                    <button type="button" className="lc-runtimecell__install" onClick={() => onUpdateAgent(runtime.id)}>
                      Update
                    </button>
                  )}
                  {/* Home's Install, here too (0.780): the row's own sentence says to press it. */}
                  {status.tag === 'NOT INSTALLED' && installCommand(runtime.id) !== undefined && onInstall !== undefined && (
                    <button
                      type="button"
                      className="lc-runtimecell__install"
                      disabled={installing !== undefined || npmMissing === true}
                      title={
                        npmMissing === true
                          ? 'Node.js is not on this machine, and this installs through npm.'
                          : installing !== undefined && installing !== runtime.id
                            ? `Waiting for the ${installing} install to finish`
                            : undefined
                      }
                      onClick={() => onInstall(runtime.id)}
                    >
                      {installing === runtime.id ? 'Installing…' : 'Install'}
                    </button>
                  )}
                </div>
              )
            })}
          </div>
          {/* An install that did not finish says why, here as on Home (0.780). */}
          {installFailure !== undefined && installing === undefined && (
            <div className="lc-settingrows">
              <div className="lc-settingrow">
                <span className="lc-settings__note" role="alert">{installFailure.what} {installFailure.next}</span>
              </div>
            </div>
          )}
          {/*
            * Colin, 2026-09-23: "is there a way to make it so the models will
            * automatically update without messing up load times or
            * interfering with the app". The models are read from each agent;
            * this keeps the two agents that never update themselves current.
            */}
          {/* A copy that is not the installed Locust leaves the machine's agents alone, and says so (0.514). */}
          {runtimeUpdates?.heldHere === true && (
            <div className="lc-settingrows">
              <div className="lc-settingrow">
                <span className="lc-settings__note">
                  This copy is not the installed Locust, so it never updates the agents on this machine on its own. The installed Locust keeps them current.
                </span>
              </div>
            </div>
          )}
          {runtimeUpdates !== undefined && runtimeUpdates.heldHere !== true && onKeepAgentsCurrent !== undefined && (
            <div className="lc-settingrows">
              <div className="lc-settingrow">
                <span className="lc-settings__note">{keepCurrentNote(runtimeUpdates.automatic)}</span>
                <button
                  type="button"
                  className={`lc-switch${runtimeUpdates.automatic ? ' is-on' : ''}`}
                  role="switch"
                  aria-checked={runtimeUpdates.automatic}
                  aria-label="Update Codex CLI and Copilot CLI on their own"
                  onClick={() => onKeepAgentsCurrent(!runtimeUpdates.automatic)}
                >
                  <span className="lc-switch__knob" />
                </button>
              </div>
            </div>
          )}
        </section>
        )}
        {/*
          * A company's own model, or one on this machine (0.357). Colin: his
          * father's company builds a model of its own and could "just insert
          * their model". Anything that speaks the OpenAI chat API runs
          * through OpenCode, listed under its own name.
          */}
        {shownPage === 'models' && onOwnModelsChanged !== undefined && (
          <section className="lc-settings__section">
            <h2 className="lc-settings__heading">Your own models</h2>
            <p className="lc-settings__lede">
              A model your company runs, or one on this machine, in every teammate&rsquo;s model list under its own name.
            </p>
            <More>
              <p>
                Anything that speaks the OpenAI chat API works: a company&rsquo;s own endpoint, vLLM, Ollama&rsquo;s /v1,
                LM Studio. Teammates reach it through OpenCode, so OpenCode needs to be installed; it is the model&rsquo;s
                name, not OpenCode&rsquo;s, that the model list and the chat bar show.
              </p>
              <p>
                A key is kept only as your system encrypts it for your account, and never shown again; it is sent to that
                address and nowhere else. Test asks the address which models it serves, and sends the model one request
                capped at a single word to see whether it can use tools — so a mistyped name, or a model that can only
                chat, is caught before a teammate tries it. A model set to chat only talks with its teammates and reads
                and changes no files.
              </p>
            </More>
            <OwnModels onChanged={onOwnModelsChanged} />
          </section>
        )}
        {/*
          * YOUR CLOUD COMPUTERS (0.761, CloudComputers.tsx). Colin, 2026-10-12: "see if theres a way that we can
          * assign any teammate to the cloud or vm". Each is the person's own GitHub Codespace, listed from GitHub.
          */}
        {shownPage === 'cloud' && (
          <section className="lc-settings__section">
            <h2 className="lc-settings__heading">Cloud computers</h2>
            {policyFeatureRefusal(policy, 'cloud') !== undefined && <ManagedPolicyNote />}
            <p className="lc-settings__lede">
              Where a teammate works when you pick Cloud computer, or Cloud for a teammate whose maker has no cloud of
              its own: a GitHub Codespace on your GitHub account, one for each repository.
            </p>
            <More>
              <p>
                Each is 2 CPUs and 8 GB. GitHub includes 60 hours a month at this size on a free account; after that it
                is $0.18 an hour, charged by GitHub, and with no payment method on GitHub it stops at the free hours
                instead. It stops itself after 10 minutes with nothing to do, keeps its files while stopped, and is
                deleted after a week unused. Locust charges nothing.
              </p>
              <p>
                A teammate there works on a copy of this folder as it was when the conversation went to the cloud, and
                runs without GitHub&rsquo;s sign-in, so nothing it does reaches GitHub. Its change comes into this folder
                only when you press Apply.
              </p>
              <p>
                Claude Code and Codex there are signed in with your own accounts, once for each computer: Sign in opens
                their maker&rsquo;s page in your browser, and you click Authorize. The sign-in stays on that computer and
                goes when it is deleted. OpenCode&rsquo;s free models need no sign-in.
              </p>
            </More>
            <CloudComputers />
          </section>
        )}
        {/*
          * YOUR GITHUB (0.720). Colin, 2026-10-10, of t3code: "a legitimate
          * github connector and their logo would be very cool". The GitHub
          * CLI's own account, signed in without a terminal (GitHubAccount.tsx).
          */}
        {shownPage === 'connectors' && (
          <section className="lc-settings__section">
            <h2 className="lc-settings__heading">GitHub</h2>
            <p className="lc-settings__lede">Teammates push and open pull requests as you, through your own GitHub CLI.</p>
            <More>
              <p>
                Locust uses the GitHub account the GitHub CLI (gh) is signed in to, the same one you use in a terminal. It
                keeps no token of its own. Sign in here and Locust starts gh&rsquo;s own sign-in: it shows a one-time code,
                you enter it on GitHub&rsquo;s page in your browser, and gh keeps the sign-in where it always does.
              </p>
              <p>
                Once signed in, Commit in a conversation can also push and open a pull request. To sign out, run gh auth
                logout in a terminal.
              </p>
            </More>
            <GitHubAccount />
          </section>
        )}
        {shownPage === 'connectors' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Connectors</h2>
          <p className="lc-settings__lede">
            {askConnectors
              ? 'Asking. A Claude Code teammate stops and asks before every connector call.'
              : 'Not asking. A teammate may use any connector your Claude Code can reach, without asking.'}
          </p>
          <More>
            <p>
              A connector is not on this machine: it acts on the service it reaches, so no permission mode
              governs it. By default a teammate may use whichever connectors your own Claude Code has, the
              way you can — the question was answered when you connected them.
            </p>
            <p>
              Switch this on and every connector call stops the run and asks you first, with the exact
              input it would send. Approve once, allow that connector for the rest of the run, or deny
              with a reason the teammate reads. It is checked when a run starts, so the next run
              follows the switch without a restart. Auto never asks either way.
            </p>
          </More>
          <div className="lc-settingrows">
            <div className="lc-settingrow">
              <span className="lc-settings__note">
                {askConnectors
                  ? 'On. Every connector call raises an approval card first.'
                  : 'Off. Connectors you have are used without asking.'}
              </span>
              <button
                type="button"
                className={`lc-switch${askConnectors ? ' is-on' : ''}`}
                role="switch"
                aria-checked={askConnectors}
                aria-label={askConnectors ? 'Stop asking before connector calls' : 'Ask before every connector call'}
                onClick={() => onAskConnectorsChange(!askConnectors)}
              >
                <span className="lc-switch__knob" />
              </button>
            </div>
          </div>
          {/* W8 (0.567): each connector's state, what to do, when it last worked, and a report to copy. */}
          <ConnectorHealth key={connectorsRead} />
        </section>
        )}
        {/*
          * ONE CONNECTOR TO EVERY AGENT (0.716). Colin, 2026-10-09, of REA, an
          * MCP server: "sure you can run connectors, or however you see fit."
          * Each agent's own command adds it (shared/connector-add.ts).
          */}
        {shownPage === 'connectors' && (
          <section className="lc-settings__section">
            <h2 className="lc-settings__heading">Add a connector</h2>
            <p className="lc-settings__lede">One MCP server, given to each AI agent you choose through that agent&rsquo;s own command.</p>
            <More>
              <p>
                Claude Code, Codex, Gemini CLI, Copilot, OpenCode and Antigravity each keep their own list of connectors,
                in their own file, and each has a command that adds one. Locust runs that command for you, the way you
                would type it, and never edits their files itself. The list above is Claude Code&rsquo;s.
              </p>
              <p>
                An agent that already has a connector by that name keeps its own, untouched: it may hold a key Locust
                never saw. One that cannot say whether it has one is left alone. Undo takes it back out with each
                agent&rsquo;s own command; OpenCode has none, so it says where its copy is kept.
              </p>
            </More>
            <AddConnector
              installed={runtimes.filter((runtime) => runtime.installed).map((runtime) => runtime.id).filter(isConnectorAgent)}
              onChanged={() => setConnectorsRead((read) => read + 1)}
            />
          </section>
        )}
        {shownPage === 'runtimes' && (
        <section className="lc-settings__section">
          {/* R9 (0.617): which AI agents ask first, so which a card can stop -- the words docs/WHAT-LOCUST-CAN-STOP.md is written from. */}
          <h2 className="lc-settings__heading">What Locust can stop</h2>
          <p className="lc-settings__lede">{WHAT_LOCUST_CAN_STOP_LEDE}</p>
          <WhatLocustCanStop />
        </section>
        )}
        {shownPage === 'runtimes' && (
        <section className="lc-settings__section" data-setting="plan-runs-out">
          {/*
            0.774 (shared/carry-on.ts): the third way on, which this section used to say was not built. 0.839: and a
            provider that failed before the turn did anything, as OpenBot's backup models do.
          */}
          <h2 className="lc-settings__heading">When an agent cannot go on</h2>
          <p className="lc-settings__lede">
            {planRunsOut === undefined
              ? 'A run whose plan runs out, or whose provider fails before it does anything, stops where it is. Its card offers to carry the work on with the first AI agent that is ready, told what was done. Pick one here to choose which.'
              : planRunsOut.automatic
                ? `${carryOnChoices.find((choice) => choice.runtime === planRunsOut.runtime)?.label ?? planRunsOut.runtime} carries the work on by itself when another agent's plan runs out, or its provider fails before it does anything, told what was done.`
                : `A run whose plan runs out, or whose provider fails before it does anything, stops where it is, and its card offers to carry the work on with ${carryOnChoices.find((choice) => choice.runtime === planRunsOut.runtime)?.label ?? planRunsOut.runtime}.`}
          </p>
          <div className="lc-carryon" role="radiogroup" aria-label="Carry on with">
            <button
              type="button"
              role="radio"
              aria-checked={planRunsOut === undefined}
              className={`lc-button${planRunsOut === undefined ? ' is-active' : ''}`}
              onClick={() => onPlanRunsOutChange?.(null)}
            >
              First one ready
            </button>
            {carryOnChoices.map((choice) => (
              <button
                key={choice.runtime}
                type="button"
                role="radio"
                aria-checked={planRunsOut?.runtime === choice.runtime}
                className={`lc-button${planRunsOut?.runtime === choice.runtime ? ' is-active' : ''}`}
                onClick={() =>
                  onPlanRunsOutChange?.(
                    planRunsOut?.runtime === choice.runtime ? planRunsOut : { runtime: choice.runtime, automatic: planRunsOut?.automatic === true }
                  )
                }
              >
                {choice.label}
              </button>
            ))}
          </div>
          <div className="lc-settingline lc-settingline--sub">
            <div className="lc-settingline__text">
              <h3 className="lc-settings__heading">Without asking</h3>
              <p className="lc-settings__lede">
                {planRunsOut === undefined
                  ? 'Pick an agent above first: Locust never moves your work on its own to an agent you did not choose.'
                  : planRunsOut.automatic
                    ? 'On. Only for messages you send yourself, never a comparison, a routine or a reply between teammates.'
                    : 'Off. The card waits for your press.'}
              </p>
            </div>
            <button
              type="button"
              className={`lc-switch${planRunsOut?.automatic === true ? ' is-on' : ''}`}
              role="switch"
              aria-checked={planRunsOut?.automatic === true}
              aria-label="Carry on without asking"
              disabled={planRunsOut === undefined}
              onClick={() => {
                if (planRunsOut !== undefined) onPlanRunsOutChange?.({ ...planRunsOut, automatic: !planRunsOut.automatic })
              }}
            >
              <span className="lc-switch__knob" />
            </button>
          </div>
          <More>
            <p>
              Wherever the work goes on, Locust carries it there with a summary of what was done and what
              was left unsettled, and a line in the conversation says where it moved and why.
            </p>
          <div className="lc-settingcard">
          <div className="lc-policyrow">
            <span className="lc-tag">HAND OFF</span>
            <span className="lc-settings__note">
              Pick another AI agent from the model menu while it is running. It is stopped there and
              carried on with a summary of what was done.
            </span>
          </div>
          <div className="lc-policyrow">
            <span className="lc-tag">CONTINUE ELSEWHERE</span>
            <span className="lc-settings__note">
              After a run has stopped, pick another model and reply. The next turn starts there with a
              summary of what was done, and your reply as its first instruction.
            </span>
          </div>
          <div className="lc-policyrow">
            <span className="lc-tag">CARRY ON</span>
            <span className="lc-settings__note">
              When a plan runs out, one press on the stopped run&rsquo;s card carries the work on with the
              agent picked above, or, with Without asking on, it goes on by itself. Claude Code and Codex
              say when their plan runs out; an agent that does not say so is not carried on. The same goes
              for a sign-in that expired, a provider that is down or no network, but only when the run had
              not used a single tool yet, so nothing is done twice.
            </span>
          </div>
          </div>
          </More>
        </section>
        )}
        {shownPage === 'runtimes' && (
        <section className="lc-settings__section" data-setting="acp-agents">
          <h2 className="lc-settings__heading">Your own agents</h2>
          <p className="lc-settings__lede">
            Any agent that speaks the Agent Client Protocol, added by the command that starts it. It joins every
            teammate&rsquo;s model list, in a group of its own.
          </p>
          <More>
            <p>
              Goose, Qwen Code, Kimi CLI, Mistral Vibe, Factory Droid and many more start in this protocol with a
              command like <span className="lc-mono">goose acp</span> or <span className="lc-mono">qwen --acp</span>;
              each says which in its own documentation. Install the agent and sign in to it as it asks first.
            </p>
            <p>
              Test starts it in an empty folder, asks its name and the models it offers, and stops it. No message is
              sent, so nothing is spent. It runs in Approve each: every action it asks to take is a card you answer.
              Which actions it asks about is the agent&rsquo;s own setting &mdash; OpenCode, for one, changes files
              without asking unless its permission settings say ask. Locust offers it no files and no terminal of its
              own, and runs the command directly, never through a shell.
            </p>
          </More>
          <AcpAgentsSettings onChanged={() => onOwnModelsChanged?.()} />
        </section>
        )}

        {shownPage === 'teammates' && (
        <section className="lc-settings__section lc-settings__section--line">
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <h2 className="lc-settings__heading">Swarm</h2>
              <p className="lc-settings__lede">
                {swarm
                  ? 'On. Every run uses its model\u2019s maximum effort.'
                  : 'Off. Each run uses the effort its model is set to.'}
              </p>
            </div>
            <button
              type="button"
              className={`lc-switch${swarm ? ' is-on' : ''}`}
              role="switch"
              aria-checked={swarm}
              aria-label="Swarm"
              onClick={() => onSwarmChange(!swarm)}
            >
              <span className="lc-switch__knob" />
            </button>
          </div>
          <More>
            <p>
              Swarm is a statement about every run rather than about one of them: while it is on, each
              run is given the highest effort the model it lands on reports, and the effort control says who
              is holding it. A model that reports no levels is unaffected — there is nothing to raise.
            </p>
            <p>
              The mark on the composer is the same switch seen from the other side. It is the glance; this
              is the record, and the way to take it back from a screen that has no composer on it — which
              is every screen but the workroom, and the conversation itself while a reply is running.
            </p>
          </More>

        </section>
        )}
        {shownPage === 'teammates' && (
        <section className="lc-settings__section lc-settings__section--line">
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <h2 className="lc-settings__heading">Auto mode</h2>
              {autoManaged && <ManagedPolicyNote />}
              <p className="lc-settings__lede">
                {autoMode
                  ? 'On. A run started in Auto goes ahead without asking and is not kept to the project folder.'
                  : 'Off. Teammates may change files inside the project folder and nowhere else.'}
              </p>
            </div>
            <button
              type="button"
              className={`lc-switch${autoMode ? ' is-on' : ''}`}
              role="switch"
              aria-checked={autoMode}
              aria-label="Auto mode"
              disabled={autoManaged}
              onClick={() => onAutoModeChange(!autoMode)}
            >
              <span className="lc-switch__knob" />
            </button>
          </div>
          <More>
            <p>
              Every other mode confines a run to the folder you chose for it, and a runtime that wants to
              touch anything outside it is refused. Auto is the mode that does not: a run started in Auto
              goes ahead without asking and may read and change files anywhere this computer lets you,
              including outside the folder.
            </p>
            <p>
              It exists because some work genuinely lives in more than one place. Auto is always in the
              composer's permission menu and picking it there is what switches it on, so this is the same
              decision seen from the other side: what is on now, and the way to take it back. It is checked
              again each time a run starts — switching it off here stops the next run, including one a
              teammate or a routine was about to start — and it is never the mode a malformed or missing
              choice falls back to.
            </p>
          </More>
        </section>
        )}
        {shownPage === 'teammates' && (
        <section className="lc-settings__section lc-settings__section--line">
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <h2 className="lc-settings__heading">Plans</h2>
              <p className="lc-settings__lede">
                {keepATodoList
                  ? 'Asking for a plan. Teammates that can keep a todo list are asked to, and the board fills in as they work.'
                  : 'Not asking. A teammate keeps a list only if it decides to on its own.'}
              </p>
            </div>
            <button
              type="button"
              className={`lc-switch${keepATodoList ? ' is-on' : ''}`}
              role="switch"
              aria-checked={keepATodoList}
              aria-label={keepATodoList ? 'Stop asking teammates to keep a todo list' : 'Ask teammates to keep a todo list'}
              onClick={() => onKeepATodoListChange(!keepATodoList)}
            >
              <span className="lc-switch__knob" />
            </button>
          </div>
          <More>
            <p>
              Codex, Cursor and OpenCode each have a tool for keeping a todo list, and the board in a room
              draws whatever they put in it. This asks them to use it: add the steps once they are known,
              mark one in progress, mark it done when it is done.
            </p>
            <p>
              It is off by default because it is not free — the bookkeeping costs tokens and changes how a
              teammate narrates itself. It is worth switching on when you are watching the board rather
              than reading every line.
            </p>
            <p>
              Claude Code is never asked. It has no such tool, so the request would be an instruction it
              cannot follow and the board would stay empty with no explanation. This switch does nothing
              for a Claude Code teammate, in either position.
            </p>
          </More>
        </section>
        )}
        {shownPage === 'teammates' && (
        <section className="lc-settings__section lc-settings__section--line" data-setting="claude-skills">
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <h2 className="lc-settings__heading">Your skills</h2>
              <p className="lc-settings__lede">
                {claudeOwnSkills
                  ? "Lent. Claude Code teammates can use your own skills as well as the folder's."
                  : "Not lent. Claude Code teammates use the folder's skills, not yours."}
              </p>
            </div>
            <button
              type="button"
              className={`lc-switch${claudeOwnSkills ? ' is-on' : ''}`}
              role="switch"
              aria-checked={claudeOwnSkills}
              aria-label={claudeOwnSkills ? 'Stop lending your skills to Claude Code teammates' : 'Lend your skills to Claude Code teammates'}
              onClick={() => onClaudeOwnSkillsChange?.(!claudeOwnSkills)}
            >
              <span className="lc-switch__knob" />
            </button>
          </div>
          <More>
            <p>
              A Claude Code teammate can use the skills in the folder's .claude/skills, the way Claude Code
              does. Your own, in .claude/skills in your home folder, were written for your own sessions, so
              they are lent only when this is on. It is read each time a run starts.
            </p>
            <p>
              A skill is instructions. It uses only the tools the teammate's mode already gives it, so a
              teammate in Ask that uses a skill still changes nothing. In Auto a teammate runs as you and
              finds every skill you have, whichever way this is set.
            </p>
            <p>
              Codex, OpenCode, Copilot and Antigravity find your own skills themselves, in the folders they
              already read in your own sessions; this switch is for Claude Code. {SKILL_RUNTIME_NAMES} teammates
              get their own skills from a tune and the ones you keep from GitHub.
            </p>
          </More>
        </section>
        )}
        {shownPage === 'teammates' && (
        <section className="lc-settings__section" data-setting="skills-from-github">
          <h2 className="lc-settings__heading">Skills from GitHub</h2>
          <p className="lc-settings__lede">
            Keep skills from a public repository. {SKILL_RUNTIME_NAMES} teammates get them on every run, in
            every mode.
          </p>
          <SkillsFromGitHub />
          <More>
            <p>
              Look lists every skill in the repository and every file each one carries, before anything is
              downloaded. Files that run, such as scripts and programs, are named. Only the skills you tick are
              kept, exactly as they are at the commit you looked at. Nothing changes when the repository does,
              until you look for changes and keep again.
            </p>
            <p>
              A skill is instructions a teammate follows, and its scripts run with the teammate's mode. In Ask
              they cannot change anything; in Auto a teammate can run them without asking. Keep skills from
              people you trust, as you would install a program.
            </p>
            <p>Only public repositories are read, and Locust sends GitHub nothing about you.</p>
          </More>
        </section>
        )}
        {shownPage === 'teammates' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Saved approvals</h2>
          <SavedApprovalRules />
        </section>
        )}
        {shownPage === 'memory' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">What your team remembers</h2>
          <p className="lc-settings__lede">
            Teammates keep a shared memory per folder, plus a smaller set marked everywhere.
          </p>
          <More>
            <p>
              The way Claude Code and Cursor do, managed from here. A teammate writes one by ending a reply
              with it; every teammate in the folder reads what is kept. Each memory says who wrote it, where,
              and from which conversation, and you can edit, switch off, or remove any of them.
            </p>
          </More>
          <div className="lc-settingrows">
          <div className="lc-settingrow">
            <span className="lc-settings__note">When a teammate writes a memory</span>
            <div className="lc-segmented" role="radiogroup" aria-label="When a teammate writes a memory">
              {(
                [
                  ['auto', 'Keep and tell me'],
                  ['ask', 'Ask me first'],
                  ['off', 'Off']
                ] as const
              ).map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  role="radio"
                  aria-checked={memoryMode === value}
                  className={`lc-button${memoryMode === value ? ' is-active' : ''}`}
                  onClick={() => onMemoryModeChange(value)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
          <div className="lc-settingrow">
            <span className="lc-settings__note">
              {memoryCount === 0 ? 'Nothing remembered yet.' : `${String(memoryCount)} ${memoryCount === 1 ? 'memory' : 'memories'} kept.`}
              {memoryWaiting > 0 ? ` ${String(memoryWaiting)} waiting for you.` : ''}
            </span>
            <button type="button" className="lc-ghostbutton" title="Ctrl 5" onClick={onOpenMemory}>
              Open memory
            </button>
          </div>
          </div>
        </section>
        )}

        {/* The theme heads Appearance: it is the one choice here that changes every screen (0.789). */}
        {shownPage === 'appearance' && (
        <section className="lc-settings__section lc-settings__section--line">
          <ThemeSetting />
        </section>
        )}

        {/*
          * READING COMFORT FIRST. The one setting here that changes how a
          * reply reads sat under the sidebar and the send button's metal
          * effects -- a hundred and twenty lines of ornament above it (the
          * design review: "Appearance: reading comfort before ornament").
          */}
        {shownPage === 'appearance' && (
        <section className="lc-settings__section lc-settings__section--line">
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <h2 className="lc-settings__heading">Reply text size</h2>
              <p className="lc-settings__lede">
                {replySize === 'largest'
                  ? 'Largest. For reading at a distance, or a long reply you want to sit with.'
                  : replySize === 'large'
                    ? 'Large. A step up without turning the thread into a slide.'
                    : 'Standard. The size a reply has always been set at.'}
              </p>
            </div>
            <div className="lc-segmented" role="radiogroup" aria-label="How big to set a reply">
              {(
                [
                  ['standard', 'Standard'],
                  ['large', 'Large'],
                  ['largest', 'Largest']
                ] as const
              ).map(([option, label]) => (
                <button
                  key={option}
                  type="button"
                  role="radio"
                  aria-checked={replySize === option}
                  className={`lc-button${replySize === option ? ' is-active' : ''}`}
                  onClick={() => onReplySizeChange(option)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
          <More>
            <p>
              A teammate's reply is the one thing on this screen you read rather than scan, so it is set
              in its own face and sized on its own. Your own messages and a teammate's question cards grow
              with it; the rest of the app stays where it is whatever you pick here.
            </p>
            <p>
              The line length follows the size rather than staying put, so a bigger reply gets a wider
              paragraph and about the same number of words to a line. Measured, not assumed: roughly 74
              characters at Standard and 72 at Large.
            </p>
          </More>
        </section>
        )}
        {shownPage === 'appearance' && (
        <section className="lc-settings__section lc-settings__section--line" data-setting="terminal-faces">
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <h2 className="lc-settings__heading">Terminal faces</h2>
              <p className="lc-settings__lede">
                {terminalFaces
                  ? "On. A teammate with a screen for a face wears it, its eyes lit code that says what it is doing. Choose each one's face in its look."
                  : 'Off. Every teammate keeps its own eyes.'}
              </p>
            </div>
            <div className="lc-settingline__preview" aria-hidden>
              {TERMINAL_FACES_PREVIEW.map(([type, eyes], i) => (
                <span key={type} className="lc-settingline__bot" data-bot={type}>
                  <Bot type={type} size={40} paused seed={0.3 + i * 0.2} eyes={eyes} />
                </span>
              ))}
            </div>
            <div className="lc-segmented" role="radiogroup" aria-label="Terminal faces">
              {(
                [
                  [true, 'On'],
                  [false, 'Off']
                ] as const
              ).map(([option, label]) => (
                <button
                  key={label}
                  type="button"
                  role="radio"
                  aria-checked={terminalFaces === option}
                  className={`lc-button${terminalFaces === option ? ' is-active' : ''}`}
                  onClick={() => onTerminalFacesChange?.(option)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
        </section>
        )}
        {shownPage === 'appearance' && (
        <section className="lc-settings__section lc-settings__section--line" data-setting="face-legend">
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <h2 className="lc-settings__heading">What a face says</h2>
              <p className="lc-settings__lede">
                {terminalFaces
                  ? "A teammate's face says what it is doing, and its eyes say it too. This one shows each in turn; click it for the next."
                  : "A teammate's face says what it is doing. This one shows each in turn; click it for the next. With Terminal faces on, its eyes say it too."}
              </p>
            </div>
            <FaceLegend />
          </div>
        </section>
        )}
        {shownPage === 'appearance' && (
        <section className="lc-settings__section lc-settings__section--line" data-setting="plush">
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <h2 className="lc-settings__heading">Plush</h2>
              <p className="lc-settings__lede">
                {plush
                  ? 'On. Every teammate is soft faux fur, with a shorter pile where it is drawn small.'
                  : 'Off. Every teammate is smooth plastic.'}
              </p>
            </div>
            <div className="lc-settingline__preview" aria-hidden>
              {PLUSH_PREVIEW.map((type, i) => (
                <span key={type} className="lc-settingline__bot" data-bot={type}>
                  <Bot type={type} size={40} paused seed={0.3 + i * 0.2} />
                </span>
              ))}
            </div>
            <div className="lc-segmented" role="radiogroup" aria-label="Plush">
              {(
                [
                  [true, 'On'],
                  [false, 'Off']
                ] as const
              ).map(([option, label]) => (
                <button
                  key={label}
                  type="button"
                  role="radio"
                  aria-checked={plush === option}
                  className={`lc-button${plush === option ? ' is-active' : ''}`}
                  onClick={() => onPlushChange?.(option)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
        </section>
        )}
        {shownPage === 'appearance' && (
        <section className="lc-settings__section lc-settings__section--line">
          {/*
            * The shell layout. It was decided by window width alone, so the
            * only way to get the compact rail was to shrink the window and the
            * only way out of it was to grow one (Colin, 2026-09-07: "can we
            * have it be an optional toggle as well?"). Auto still follows the
            * width, and it is what everyone gets until they choose.
            */}
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <h2 className="lc-settings__heading">Sidebar</h2>
              <p className="lc-settings__lede">
                {layout === 'auto'
                  ? `Auto. Following this window, currently ${layoutMode === 'wide' ? 'full' : 'the rail'}.`
                  : layout === 'wide'
                    ? 'Full. The names beside the faces, whatever the window.'
                    : 'Rail. Faces only, whatever the window.'}
              </p>
            </div>
            <div className="lc-segmented" role="radiogroup" aria-label="Sidebar layout">
              {(
                [
                  ['auto', 'Auto'],
                  ['wide', 'Full'],
                  ['compact', 'Rail']
                ] as const
              ).map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  role="radio"
                  aria-checked={layout === value}
                  className={`lc-button${layout === value ? ' is-active' : ''}`}
                  onClick={() => onLayoutChange(value)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
        </section>
        )}
        {shownPage === 'appearance' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Send button</h2>
          <p className="lc-settings__lede">
            {metal === 'off'
              ? 'Off. A plain button, and no shader running in the composer.'
              : metalMotion === 'always'
                ? `Always on. ${metal === 'chromatic' ? 'Chromatic' : metal === 'silver' ? 'Silver' : 'Gold'}, moving whether or not you are there.`
                : `On hover. ${metal === 'chromatic' ? 'Chromatic' : metal === 'silver' ? 'Silver' : 'Gold'}, still until you point at it.`}
          </p>
          <More>
            <p>
              Every option here is one the design pass put forward, on screen rather than buried in the
              source, because a look is settled by seeing it rather than by describing it.
            </p>
            <p>
              Its recommendation was silver at Standard, on hover only — the argument being that in this
              app a thing that moves means work is happening, so a button that shimmers all the time says
              "running" on a screen where nothing is. Always on is here so you can disagree with that
              after looking at it.
            </p>
            <p>
              Off is not only a matter of taste: the effect is WebGL, and a plain button costs nothing and
              cannot fail to appear.
            </p>
          </More>
          <div className="lc-settingrows">
            <div className="lc-settingrow">
              <span className="lc-settings__note">Which metal</span>
              <div className="lc-segmented" role="radiogroup" aria-label="Which metal">
                {(
                  [
                    ['off', 'Off'],
                    ['chromatic', 'Chromatic'],
                    ['silver', 'Silver'],
                    ['gold', 'Gold']
                  ] as const
                ).map(([option, label]) => (
                  <button
                    key={option}
                    type="button"
                    role="radio"
                    aria-checked={metal === option}
                    className={`lc-button${metal === option ? ' is-active' : ''}`}
                    onClick={() => onMetalChange({ metal: option })}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>
            <div className="lc-settingrow">
              <span className="lc-settings__note">How strong</span>
              <div className="lc-segmented" role="radiogroup" aria-label="How strong the metal is">
                {(
                  [
                    ['subtle', 'Subtle'],
                    ['standard', 'Standard'],
                    ['strong', 'Strong']
                  ] as const
                ).map(([option, label]) => (
                  <button
                    key={option}
                    type="button"
                    role="radio"
                    aria-checked={metalStrength === option}
                    disabled={metal === 'off'}
                    className={`lc-button${metalStrength === option ? ' is-active' : ''}`}
                    onClick={() => onMetalChange({ metalStrength: option })}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>
            <div className="lc-settingrow">
              <span className="lc-settings__note">When it moves</span>
              <div className="lc-segmented" role="radiogroup" aria-label="When the metal moves">
                {(
                  [
                    ['hover', 'On hover'],
                    ['always', 'Always on']
                  ] as const
                ).map(([option, label]) => (
                  <button
                    key={option}
                    type="button"
                    role="radio"
                    aria-checked={metalMotion === option}
                    disabled={metal === 'off'}
                    className={`lc-button${metalMotion === option ? ' is-active' : ''}`}
                    onClick={() => onMetalChange({ metalMotion: option })}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>
            <div className="lc-settingrow">
              <span className="lc-settings__note">
                Cursor bend: a liquid dent that rides the ring as you move across it
              </span>
              <div className="lc-segmented" role="radiogroup" aria-label="The cursor bend">
                {(
                  [
                    [true, 'On'],
                    [false, 'Off']
                  ] as const
                ).map(([option, label]) => (
                  <button
                    key={String(option)}
                    type="button"
                    role="radio"
                    aria-checked={metalBend === option}
                    disabled={metal === 'off'}
                    className={`lc-button${metalBend === option ? ' is-active' : ''}`}
                    onClick={() => onMetalChange({ metalBend: option })}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>
          </div>
        </section>
        )}
        {shownPage === 'appearance' && (
        <section className="lc-settings__section lc-settings__section--line">
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <h2 className="lc-settings__heading">Boot screen</h2>
              <p className="lc-settings__lede">
                {tube === 'off'
                  ? 'Off. Locust goes straight to the workspace while it finds your AI agents.'
                  : tube === 'subtle'
                    ? 'Subtle. The screen without the flicker or the glare.'
                    : 'Full. The whole monitor while your AI agents are found.'}
              </p>
            </div>
            {/* `lc-button is-active` is what every other segmented choice
              * in this app already uses -- inventing a class for this one
              * would be a second spelling of a solved thing. */}
            <div className="lc-segmented" role="radiogroup" aria-label="How much of the boot screen to draw">
              {(
                [
                  ['full', 'Full'],
                  ['subtle', 'Subtle'],
                  ['off', 'Off']
                ] as const
              ).map(([option, label]) => (
                <button
                  key={option}
                  type="button"
                  role="radio"
                  aria-checked={tube === option}
                  className={`lc-button${tube === option ? ' is-active' : ''}`}
                  onClick={() => onTubeChange(option)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
          <More>
            <p>
              Finding the AI agents on this machine takes as long as it takes — each one is a real command
              and some of them are slow to answer. The screen shows that happening rather than a spinner
              standing in for it, and every line on it is something the app actually read.
            </p>
            <p>
              It only ever covers the empty middle of the window. The sidebar, the folder and the message
              box stay where they are, so you can pick a teammate or start typing without waiting for it,
              and clicking anywhere on it puts it away.
            </p>
          </More>

        </section>
        )}

        {shownPage === 'app' && (
        <section className="lc-settings__section lc-settings__section--line" data-setting="locust-mcp">
          <OrganizationPolicySettings heading={<h2 className="lc-settings__heading">Managed by your organization</h2>} />
          <LocustMcpSettings heading={<h2 className="lc-settings__heading">Let your other AI apps use Locust</h2>} />
        </section>
        )}
        {shownPage === 'app' && (
        <section className="lc-settings__section lc-settings__section--line" data-setting="phone">
          <PhoneSettings
            heading={<h2 className="lc-settings__heading">Your phone</h2>}
            more={
              <More>
                From anywhere, this computer keeps one connection open to Locust’s relay, and your phone reaches it
                there from any network. Each message is locked on one side and opened on the other, with keys only this
                computer and your phone hold, so the relay passes along what it cannot read. Only on this Wi-Fi, Locust
                opens one small page on this computer for your home network instead, and nothing goes through anyone
                else’s server. Either way, a message you send from your phone is your next message in that
                conversation, on the same teammate and mode, as if you typed it here, and turning this off unpairs
                every phone.
              </More>
            }
          />
        </section>
        )}
        {shownPage === 'app' && (
        <section className="lc-settings__section lc-settings__section--line" data-setting="chat-apps">
          <ChatAppsSettings
            heading={<h2 className="lc-settings__heading">Chat apps</h2>}
            more={
              <More>
                Each app is your own: a bot you make in Discord or Telegram, or an app you make in Slack, and its token
                stays encrypted on this computer. While Locust is open, it keeps one connection out to that app -- Discord’s
                gateway, Slack’s Socket Mode, or Telegram’s long requests -- so nothing on this computer listens for it.
                Only the account that paired, by the code or the link, is answered; anyone else who finds the bot is
                ignored. A message there is your next message to that teammate, on its own model and mode, as if you typed
                it here, and you see it here too. A card from a turn started there comes to the chat with Allow once and
                Deny. A message that arrives while Locust is closed starts nothing when it opens: the chat says so, and you
                send it again.
              </More>
            }
          />
        </section>
        )}
        {shownPage === 'app' && (
        <section className="lc-settings__section lc-settings__section--line" data-setting="voice-typing">
          <div className="lc-settingline">
            <div className="lc-settingline__text">
              <h2 className="lc-settings__heading">Voice typing</h2>
              <p className="lc-settings__lede">Fast is local, with a 41 MB first-use download. Accurate is local, with a 69 MB first-use download, and is slower. Your OpenAI account sends audio to OpenAI using your API key; API charges apply.</p>
            </div>
            <VoiceSettings />
          </div>
        </section>
        )}
        {shownPage === 'app' && <TalkVoice />}
        {shownPage === 'app' && <TalkShortcutSetting heading={<h2 className="lc-settings__heading">Talk from any app</h2>} />}
        {shownPage === 'app' && <ScoutSetting heading={<h2 className="lc-settings__heading">Scout keeps watch</h2>} state={scout}
          {...(scoutModel === undefined ? {} : { model: scoutModel })} {...(scoutRoute === undefined ? {} : { route: scoutRoute })}
          {...(checkCommand === undefined || checkCommand.trim().length === 0 ? {} : { checkCommand: checkCommand.trim() })}
          {...(onChangeScoutModel === undefined ? {} : { onChangeModel: onChangeScoutModel })} {...(scoutTeam === undefined ? {} : { team: scoutTeam })} />}
        {shownPage === 'app' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Startup</h2>
          <p className="lc-settings__lede">
            Routines run while Locust is open. Signing in can open it in the tray, with no window, until you ask for one.
          </p>
          <div className="lc-settingrows">
            <div className="lc-settingrow">
              <span className="lc-settings__note">Start Locust when you sign in to Windows.</span>
              <button
                type="button"
                className={`lc-switch${loginItem.openAtLogin ? ' is-on' : ''}`}
                role="switch"
                aria-checked={loginItem.openAtLogin}
                aria-label="Start Locust when you sign in to Windows."
                disabled={loginItem.available !== true}
                {...(loginItem.available === true
                  ? {}
                  : { title: loginItem.why === 'platform' ? 'Windows only for now.' : 'Only the installed app can sign in with Windows. A development copy would register Electron.' })}
                onClick={() => onLoginItemChange?.(loginItem.openAtLogin !== true)}
              >
                <span className="lc-switch__knob" />
              </button>
            </div>
            <div className="lc-settingrow">
              <span className="lc-settings__note">Keep Locust running in the background when the window is closed.</span>
              <button
                type="button"
                className={`lc-switch${keepRunning ? ' is-on' : ''}`}
                role="switch"
                aria-checked={keepRunning}
                aria-label="Keep Locust running in the background when the window is closed."
                onClick={() => onKeepRunningChange?.(keepRunning !== true)}
              >
                <span className="lc-switch__knob" />
              </button>
            </div>
            <div className="lc-settingrow">
              <span className="lc-settings__note">Keep this computer awake while it is plugged in, so routines and Scout run on time.</span>
              <button
                type="button"
                className={`lc-switch${stayAwake ? ' is-on' : ''}`}
                role="switch"
                aria-checked={stayAwake}
                aria-label="Keep this computer awake while it is plugged in, so routines and Scout run on time."
                onClick={() => onStayAwakeChange?.(stayAwake !== true)}
              >
                <span className="lc-switch__knob" />
              </button>
            </div>
          </div>
          {stayAwake && (
            <p className="lc-settings__note">
              Only the sleep that comes when nobody uses it is held off, while Locust is open. The screen still turns off,
              and Sleep and closing the lid work as they always do. On battery it sleeps as usual.
            </p>
          )}
          {loginItem.available !== true && (
            <p className="lc-settings__note">
              {loginItem.why === 'platform' ? 'Windows only for now.' : 'Only the installed app can sign in with Windows. A development copy would register Electron.'}
            </p>
          )}
        </section>
        )}
        {shownPage === 'app' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Updates</h2>
          <p className="lc-settings__lede">
            Locust checks for a new version on its own and downloads it quietly.
          </p>
          <More>
            <p>
              It never installs one while a teammate is working: restarting then would cut the run off and
              leave its record without a receipt.
            </p>
          </More>
          <UpdateControl update={update} onCheck={onCheckUpdate} onInstall={onInstallUpdate} />
          {/*
            * WHICH BUILDS (0.307). Off: the releases -- GitHub's latest. On:
            * test builds too, as they are published (main/update-lane.ts).
            *
            * It said "One new build a day, the one testers get" (0.307). Then
            * Colin set the cadence: until there are real users, every verified
            * build IS the release, and releases are batched once users arrive.
            * Under that the sentence was false -- off took every build too.
            * These words hold under either cadence (0.310); whether the switch
            * stays at all is for the next outside review.
            *
            * Then two words (0.311). Colin, with a frame of the 0.310 row:
            * "maybe switch this button to test build or beta build, this is
            * wayyy too wordy". A switch's label names the setting; the switch
            * says whether it is on.
            */}
          {onUpdateLane !== undefined && update !== undefined && update.phase !== 'unsupported' && (
            <div className="lc-settingrows">
              <div className="lc-settingrow">
                <span className="lc-settings__note">Beta builds</span>
                <button
                  type="button"
                  className={`lc-switch${update.everyBuild === true ? ' is-on' : ''}`}
                  role="switch"
                  aria-checked={update.everyBuild === true}
                  aria-label="Beta builds"
                  onClick={() => onUpdateLane(update.everyBuild !== true)}
                >
                  <span className="lc-switch__knob" />
                </button>
              </div>
            </div>
          )}
          <p className="lc-settings__note">
            This is {changelog?.version ?? 'this version'}. Every version, and what it changed, is in{' '}
            <button type="button" className="lc-linkbutton" onClick={() => setPage('whatsnew')}>
              What&rsquo;s new
            </button>
            .
          </p>
        </section>
        )}
        {shownPage === 'privacy' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Privacy &amp; local data</h2>
          <p className="lc-settings__lede">
            Every conversation is kept on this machine, in a record that is only ever added to. Nothing is uploaded.
          </p>
          <dl className="lc-receipt lc-receipt--flush">
            <dt>Record</dt>
            <dd className="lc-mono">{ledgerPath ?? 'in this profile'}</dd>
            <dt>On disk</dt>
            <dd className="lc-mono">
              {storage === undefined
                ? 'measuring…'
                : `${String(storage.missionCount)} turn${storage.missionCount === 1 ? '' : 's'} · ${formatBytes(storage.byteTotal)}${
                    storage.oldestUpdatedAt === undefined
                      ? ''
                      : ` · oldest ${new Date(storage.oldestUpdatedAt).toLocaleDateString()}`
                  }`}
            </dd>
            <dt>Network</dt>
            {/*
              * All of it (the beta handover's claims check, 2026-09-23): the
              * window asks nothing, but the app does -- its own updates, and
              * Codex's and Copilot's (Settings > Runtimes has that switch).
              */}
            {/*
              * 0.618 (the PRD's R22): it left out the pet gallery and the agents Locust installs,
              * two of its own connections. docs/NETWORK.md lists every one, and a test keeps it so.
              */}
            {/*
              * One line, then the rest behind a row you open (0.743): it was a fourteen-line paragraph in a column
              * of short values, the longest thing on any Settings page. Claude's own Privacy page keeps the detail
              * behind "How we use your data ›".
              */}
            <dd>
              The window makes no requests of its own; Locust&rsquo;s own connections are each listed.
              <details className="lc-settings__fold">
              <summary>What reaches the network</summary>
              <p>
              Locust checks for and downloads its own updates,
              installs the AI agents you ask it to and keeps Codex CLI and Copilot CLI current, reads the
              pet gallery when you open it, and asks the address of a model of your own about it when you
              press Test. Voice typing downloads its checked files only when you choose Download;
              local recordings stay on this computer. If you choose Your OpenAI account and allow it, voice typing
              sends recordings to api.openai.com using your API key. A voice call you start with Talk, beside the microphone, connects this window to OpenAI's voice service for as long as it lasts; Codex places it with your ChatGPT sign-in, and the call can use your online connectors, asking before one changes anything. Each AI agent talks to its own service. A web page a teammate made
              may load libraries and fonts from the common public hosts and reach no other site, so what it
              reads in your folder cannot be sent anywhere. <NetworkListLink />
              </p>
              </details>
            </dd>
          </dl>
          <More>
            <p>
              Nothing here is ever deleted on a timer. Turns go when you ask, after you have been shown
              exactly what would go, and a turn an ongoing conversation continues from is kept even
              when it is old.
            </p>
          </More>
          <RetentionControl report={storage} onPreview={onPreviewPrune} onPrune={onPrune} />
        </section>
        )}
        {shownPage === 'privacy' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Trash</h2>
          <p className="lc-settings__lede">
            Deleting a conversation takes it out of every list at once. The record itself waits here, so a
            deletion you did not mean can be undone.
          </p>
          <TrashControl onList={onListTrash} onRestore={onRestoreMission} onEmpty={onEmptyTrash} />
        </section>
        )}
        {shownPage === 'privacy' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Back up and restore</h2>
          <p className="lc-settings__lede">
            A backup is a folder you choose, holding your teammates, routines, memories, saved approvals and
            conversations. Your own model keys are never in it.
          </p>
          <BackupControl
            onPick={onPickProfileFolder}
            onBackUp={onBackUpProfile}
            onPreview={onPreviewProfileRestore}
            onRestore={onRestoreProfile}
          />
        </section>
        )}
        {shownPage === 'app' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Report a problem</h2>
          <p className="lc-settings__lede">
            If Locust crashes or behaves oddly, send feedback and say what happened. The log below helps.
          </p>
          <ProblemReport />
        </section>
        )}
        {shownPage === 'app' && (
        <section className="lc-settings__section">
          {/* Where a person reaches Locust's makers (0.593, the PRD's R23): one private address, the public bug page, and what each carries. */}
          <h2 className="lc-settings__heading">Help</h2>
          <p className="lc-settings__lede">
            Stuck, or something looks wrong? Two ways to reach the people who make Locust.
          </p>
          <dl className="lc-receipt lc-receipt--flush">
            <dt>Email</dt>
            <dd className="lc-mono">{SUPPORT_ADDRESS}</dd>
            <dt>Who reads it</dt>
            <dd>Only Locust&rsquo;s makers. Send feedback above fills the email in: what you wrote, your Locust version and system, and a conversation only if you include it.</dd>
            <dt>Bug reports</dt>
            <dd>Send feedback above also opens a GitHub issue, filled in the same way. Issues are public and need a GitHub account.</dd>
            <dt>Long reports</dt>
            <dd>An email cuts off a long conversation. Save as a file keeps all of it; attach the file to the email or the issue.</dd>
          </dl>
        </section>
        )}

        {shownPage === 'whatsnew' && (
        <section className="lc-settings__section">
          <h2 className="lc-settings__heading">Changelog</h2>
          <p className="lc-settings__lede">Every version of Locust, newest first. This is {changelog?.version ?? 'the one running'}.</p>
          <WhatsNew changelog={changelog} />
        </section>
        )}
        </div>
      </div>
    </div>
  )
}


/*
 * WHAT A RESTART TO INSTALL LOOKS LIKE, said before and after the click
 * (0.406). The install is silent: Locust closes, the installer replaces it
 * with nothing on screen, and the app opens again by itself. On 2026-09-27
 * that took a few minutes, and Colin opened Locust three times meanwhile --
 * getting the OLD version each time, which read as "my update doesn't seem to
 * be working" (his app's own log: 0.402 at 11:28, 11:29, 11:30; 0.405 at
 * 11:31). The installer is the assisted kind, so a visible one would show its
 * wizard; the words are what can change.
 */
export const INSTALL_TAKES = 'Locust closes while it installs and opens again by itself in a minute or two.'
export const installingLine = (version: string | undefined): string =>
  `Installing Locust${version === undefined || version === '' ? '' : ` ${version}`}. It opens again by itself in a minute or two; there is no need to open it.`

/**
 * A NEWER LOCUST FOR THIS MAC (0.515, main/mac-release.ts). A Mac copy cannot
 * update itself until the build is signed; this says when one is out and
 * hands over the download. A tester on a Mac, 2026-10-01: "oh no updates
 * also, thats kind of rough".
 */
export function MacUpdateBanner({
  release,
  onDownload
}: {
  readonly release: { readonly version: string; readonly url: string } | undefined
  readonly onDownload: (url: string) => void
}): ReactElement | null {
  if (release === undefined) return null
  return (
    <div className="lc-updatebanner" role="status">
      <span className="lc-updatebanner__text">
        {`Locust ${release.version} is out. On a Mac it is installed by hand for now: open the download and drag Locust into Applications to replace this one. Your conversations stay.`}
      </span>
      <button type="button" className="lc-button" onClick={() => onDownload(release.url)}>
        Download
      </button>
    </div>
  )
}

/**
 * The one line that says a new version is here. Settings knew; the person
 * did not, because nobody opens Settings to find out. It sits above the
 * composer, offers the install, and never installs on its own -- a running
 * mission must not be cut off mid-run, and the host refuses if one is.
 */
export function UpdateBanner({
  update,
  onInstall
}: {
  readonly update: AppUpdateState | undefined
  readonly onInstall: () => Promise<AppUpdateResponse>
}): ReactElement | null {
  const [busy, setBusy] = useState(false)
  const [refusal, setRefusal] = useState<string>()
  const [installing, setInstalling] = useState(false)
  if (update === undefined || update.phase !== 'ready') return null
  return (
    <div className="lc-updatebanner" role="status">
      <span className="lc-updatebanner__text">
        {installing ? installingLine(update.availableVersion) : `Locust ${update.availableVersion ?? ''} is downloaded and ready. ${INSTALL_TAKES}`}
        {refusal !== undefined && <span className="lc-tone-amber"> {refusal}</span>}
      </span>
      <button
        type="button"
        className="lc-button"
        disabled={busy}
        onClick={() => {
          setBusy(true)
          setRefusal(undefined)
          void onInstall()
            .then((response) => {
              if (!response.ok) setRefusal(response.error.message)
              else setInstalling(true)
            })
            .finally(() => setBusy(false))
        }}
      >
        Restart and install
      </button>
    </div>
  )
}
