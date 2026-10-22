import { accountFromRoute } from '../shared/agent-accounts.js'
import { policyFeatureRefusal, policyTurnRefusal } from '../shared/organization-policy.js'
import { supportsPerRunBrowserMcp } from '../shared/browser.js'
import type { BrowserMcpRegistration, BrowserRunContext } from './browser-host.js'
import {
  asProcessNormalizer,
  codexAppServerPolicy,
  COPILOT_ACP_SESSION,
  createAcpEventNormalizer,
  createAppServerEventNormalizer,
  createClaudeEventNormalizer,
  createClaudePrintCommand,
  createCodexAppServerCommand,
  createCodexEventNormalizer,
  createCodexExecCommand,
  createAcpAgentCommand,
  createCopilotAcpCommand,
  createCopilotEventNormalizer,
  createCopilotPromptCommand,
  createCursorEventNormalizer,
  createCursorPrintCommand,
  createAgyEventNormalizer,
  createAgyPrintCommand,
  createMuseEventNormalizer,
  createMuseExecCommand,
  createOpenCodeEventNormalizer,
  createOpenCodeRunCommand,
  createOpenCodeServeCommand,
  createOpenCodeAcpCommand,
  openCodeUsesAcp,
  cursorCanEnforceReadOnly,
  notificationOfRecord,
  startAcpRun,
  createNodeRuntimeProcessRunner,
  startCodexAppServerRun,
  startOpenCodeServeRun,
  startOpenCodeAcpRun
} from '@teammate/runtime-adapters'
import type {
  AppServerRunProcess,
  ClaudeEventNormalizer,
  ExecutableLaunch,
  CodexEventNormalizer,
  JsonValue,
  MissionRuntimeId,
  RuntimeDiscovery,
  RuntimeProcessRun,
  RuntimeProcessRunner, NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import { workspaceIdFor } from './workspace.js'
import { codexReadOnlyThreadConfig, codexSealedThreadConfig } from './codex-read-only.js'
import { cursorChoiceFor, cursorModelsFrom } from './model-catalog.js'
import { createStreamedEventBatcher } from './streamed-event-batches.js'
import { deliverWhenRuntimeStarts } from './runtime-delivery.js'
import { persistEventUpdates } from './durable-event-updates.js'
import type { MissionContinuation, MissionLedger, RecoveredMission, Workroom, WorkroomMessage } from '@teammate/mission-store'
import type { AcpCapabilities, GitReadsSpec, MissionSandbox, OpenCodeProvider, RuntimeCommandInfo, RuntimeCommandSpec } from '@teammate/runtime-adapters'
import { createHash, randomUUID } from 'node:crypto'
import { existsSync } from 'node:fs'
import { isAbsolute } from 'node:path'
import type {
  CodexMissionCancelResponse,
  CodexMissionStartResponse,
  CodexMissionUpdate,
  HandoffPreview,
  MissionApprovalAnswer,
  MissionHandoffResponse,
  MissionMode
} from '../shared/ipc.js'
import type { StartPhase } from '../shared/ipc.js'
import { acpAnswerFor, acpPermissionRequest, openCodePermissionRequest, openCodeReplyFor, withDeclines, withFileChanges } from './approval-channel.js'
import type { EditCheckResult } from './edit-check.js'
import type { CheckedTurn } from './check-fix.js'
import type { TuneSkill } from '../shared/tune.js'
import type { ApprovalChannel } from './approval-channel.js'
import { fileChangesOf, itemOf } from './approval-patch.js'
import type { FileChangeRecord } from './approval-patch.js'
import { composeColdFollowUp, composeHandoffPrompt, howTurnEnded } from './handoff.js'
import type { EarlierTurn } from './handoff.js'
import { changedPaths, observedEditEvents, observedPatches, sharedTreeNotice, snapshotWorkspace, unreportedPaths } from './disk-observation.js'
import type { Checkpoint, Checkpoints, TurnRecords } from './checkpoints.js'
import { startTimingNote } from './start-timing.js'
import type { StartTiming } from './start-timing.js'
import type { RecentEdits } from './recent-edits.js'
import { cursorCannotSee, cursorIgnoreNotice } from './cursor-visibility.js'
import type { WorkspaceSnapshot } from './disk-observation.js'
import type { ToolPatch } from '@teammate/runtime-adapters'
import { createPeerExchange, createTranscriptTracker, publicPeerMessage } from './peer-exchange.js'
import type { MemoryBriefing } from './peer-exchange.js'
import type { PeerExchange, TranscriptTracker } from './peer-exchange.js'
import type { MissionPeerContext } from './workroom-briefing.js'
import { openCodeReadOnlySection, planSection, referencesSection } from './workroom-briefing.js'
import { referenceAccess } from '../shared/references.js'
import type { ReferenceStore } from './conversation-references.js'
import type { ClaudeContextReader } from './claude-context.js'
import { undoneNote } from './undone-note.js'
import { briefHeld, briefPlan, compactedDuring } from './brief-sessions.js'
import type { BriefSessions } from './brief-sessions.js'
import type { EndedMission, RelayOrigin, SharingMission } from './relay.js'
import type { MissionStarter } from '@teammate/mission-store'
import { boundedEditCheck, reconcileMission } from '@teammate/mission-store'
import { recordableCommand } from './command-record.js'
import { commandTooLong } from './command-length.js'
import { MAX_LIVE_MISSIONS } from '../shared/live-missions.js'
import { hostReadsEventsOf, runtimeDisplayName } from '../shared/runtimes.js'
import { parsedAcpRouteModel, type AcpAgent } from '../shared/acp-agents.js'
import type { AcpLaunch } from './acp-agents.js'
import { openCode2Launch, OPENCODE_2_MODE } from './opencode-2-launch.js'
import { openCodeSkillPaths } from './opencode-skill-paths.js'

/**
 * The model and effort to hand `agy` (0.540). The app route chose a TIER
 * (flash, pro, flash_lite), and a teammate saved on one keeps it; the CLI
 * takes a model and `--effort`. A saved id with its effort on the end
 * (`gemini-3.8-flash-low`, as `agy models` lists them) is split the same way,
 * and an effort chosen in the box wins over one in the name.
 */
export function agyRoute(model: string | undefined, effort: string | undefined): { readonly model?: string; readonly effort?: string } {
  const tiers: Readonly<Record<string, { model: string; effort: string }>> = {
    flash: { model: 'gemini-3.8-flash', effort: 'medium' },
    pro: { model: 'gemini-3.1-pro', effort: 'high' },
    flash_lite: { model: 'gemini-3.8-flash', effort: 'low' }
  }
  if (model === undefined) return effort === undefined ? {} : { effort }
  const tier = Object.hasOwn(tiers, model) ? tiers[model] : undefined
  const variant = /^(.*)-(low|medium|high|max)$/.exec(model)
  const base = tier?.model ?? (variant === null ? model : variant[1]!)
  const named = tier?.effort ?? (variant === null ? undefined : variant[2])
  /*
   * A GEMINI MODEL ALWAYS GOES WITH AN EFFORT (0.665). agy refuses one without:
   * "--model gemini-3.8-flash requires --effort (available: low, medium,
   * high)". The picker always sends one, but a teammate saved with the bare
   * model id did not -- Colin's Flash, answering Bro automatically, failed so
   * on 10/05. Medium, agy's own middle, where nothing was chosen.
   */
  const chosen = effort ?? named ?? (/^gemini-/.test(base) ? 'medium' : undefined)
  return { model: base, ...(chosen === undefined ? {} : { effort: chosen }) }
}
import { FREE_ONLY_REFUSAL, isFreeRoute } from './free-routes.js'
import { attachmentsPlacedForRun } from './attachments-for-run.js'
import { longTaskFile, longTaskFilePath } from './long-task-file.js'
import { withAttachments } from '../shared/attachments.js'
import type { ReadablePdf } from '../shared/attachments.js'
import { CODEX_INIT_PROMPT, commandNamed } from './runtime-commands.js'
import type { CursorDefaultModel } from './cursor-default-model.js'
import type { PreparedSkills } from './claude-skills.js'
import type { PreparedRunSkills } from './run-skills.js'
import { checkpointMessage, checkpointNotice, checkpointSentence, turnOutcomeOf } from './turn-checkpoint.js'
import type { TurnOutcome } from './turn-checkpoint.js'
import type { CheckpointMessage, CheckpointResult } from './worktrees.js'
import type { CloudTurns, ServeSpawn } from './cloud-turns.js'
import { cloudAppServerSpawn, cloudLaunchOf, cloudProcessSpawn } from './cloud-computer.js'

export const MAX_PROMPT_LENGTH = 8_000
/**
 * Missions run side by side now, one per teammate.
 *
 * The number moved to shared/ so the room screen can say it before a person
 * builds a room bigger than it -- re-exported here because it has been part
 * of this module's surface since the cap existed.
 */
export { MAX_LIVE_MISSIONS } from '../shared/live-missions.js'
/** Owner key for a mission that belongs to nobody; those still run one at a time. */
const NOBODY = ''

interface ActiveCodexMission {
  readonly account?: string
  /** Whether this mission has already explained a refused read. Once is enough. */
  saidWhyAReadFailed: boolean
  /** Whether this run has already said a connector needs signing in to. */
  saidConnectorsNeedLogin?: boolean
  readonly runId: string
  readonly missionId: string
  readonly controller: AbortController
  readonly normalizer: CodexEventNormalizer | ClaudeEventNormalizer
  readonly process: RuntimeProcessRun
  /** Adds a message to the running turn without stopping it (A2.10); only an app-server run can. */
  readonly steer: ((text: string) => Promise<boolean>) | undefined
  readonly emit: (update: CodexMissionUpdate) => void
  /**
   * Kept so a handoff can brief the next runtime. The ledger holds this too,
   * but re-reading the file to find out what the user asked for would make a
   * switch depend on a disk read that can fail after the run is already gone.
   */
  readonly prompt: string
  readonly runtime: MissionRuntimeId
  /** Who this mission belongs to and who it may share with; absent for a mission of nobody's. */
  readonly peer: MissionPeerContext | undefined
  /** A comparison's column (0.441): the run slot it holds in place of its owner's. */
  readonly slot?: string
  /** Assistant text as the transcript rebuilt it, read for share blocks at the end. */
  readonly transcript: TranscriptTracker
  /** What this run may touch and which model it runs, so a relayed reply inherits both. */
  readonly sandbox: MissionSandbox
  readonly model: string | undefined
  /** Set when the host started this run for a teammate replying on their own. */
  readonly relay: RelayOrigin | undefined
  /** External-app turns may share visibly, but must not start automatic handoffs. */
  readonly startedBy?: MissionStarter
  /**
   * `git status` before the process started, for a run allowed to write.
   * Compared with the tree after it ends, so an edit the runtime made through
   * a sub-agent and never reported still gets a row (OpenCode's `task` tool,
   * 2026-09-05). Undefined outside a repository, or for a read-only run.
   */
  readonly diskBefore: WorkspaceSnapshot | undefined
  /** The folder as Locust kept it before the run (0.674, undo a turn); undefined when none was kept. */
  readonly checkpointBefore: Checkpoint | undefined
  /** Where the run happened: the teammate's worktree, or the folder. */
  readonly cwd: string
  /**
   * The project folder the conversation runs in, by id (0.813): what its check after edits, and what that check's
   * failure may do, are read for. Not the window's folder, which the person may have switched since (0.458).
   */
  readonly workspaceId: string
  /** The turn this one continues, so a second next turn of it is known while this one runs (0.813). */
  readonly continues?: string
  /**
   * Set when another writing run shared this folder while this one was live.
   *
   * The disk observation below is a before/after comparison of the WHOLE
   * working tree, so it cannot tell one run's writes from another's. Three
   * teammates started at once in one folder each wrote a file, and each run's
   * activity card then claimed all three and the sum of their lines -- "3
   * files, +124" for a single forty-line poem (drive, 2026-09-06). The tree
   * is shared on purpose; the attribution is what has to give way.
   *
   * Marked on BOTH runs the moment they overlap, because by the time either
   * finishes the other may be gone and the overlap invisible.
   */
  sharedTree: boolean
  /**
   * Whether this run's PROCESS has exited.
   *
   * Distinct from being out of `active`, and the distinction is the whole of
   * a defect the 0.36.2 shared-folder mark shipped with. A finished run stays
   * in `active` for a long time after its process is gone: through its own
   * disk observation, through its share to the workroom, and through the
   * relay's `onShared` -- which STARTS the recipient's run and is awaited
   * before `clearActive`. So every relayed reply found the sender still
   * "active", both were marked as sharing the folder, and the reply's own
   * edit was then counted against nobody, about a process that had already
   * exited (QA, 2026-09-06). The same happened to any run a person started
   * while a finished run's `git status` was still being taken.
   *
   * Two runs share a folder when both are RUNNING in it. This is how that
   * question gets asked.
   */
  settled: boolean
  /** The last event sequence persisted, so a synthetic event can follow it. */
  lastSequence: number
  /** Every event persisted, so the observation can tell reported edits from unreported ones. */
  readonly persisted: NormalizedRuntimeEvent[]
  /** Called only for durable runtime records, never the host's terminal receipt. */
  readonly deliverMessages: (events: readonly NormalizedRuntimeEvent[]) => Promise<void>
  /** When the start's phases began and the first event arrived (0.602); the run's end writes the note. */
  readonly startTiming: StartTiming
  /** Approval cards flush text already normalized by this consume loop. */
  flushEvents?: () => Promise<void>
}

/**
 * What a question asked ON THE SIDE is told (0.461): it is a copy of the
 * conversation, the conversation carries on without it, and nothing is to
 * change. Short, because the forked session already holds everything else.
 */
export const SIDE_QUESTION_PREFACE =
  'This is a side question from the person, asked on a copy of this conversation while the conversation itself carries on. Answer it from what you already know and what you can read. Change nothing and start nothing.'
/** How much of a running turn's ask a side question carries (0.515): enough to know the task. */
export const SIDE_RUNNING_ASK_CHARS = 2_000

export interface CodexMissionService {
  start(
    prompt: unknown,
    runtime: MissionRuntimeId,
    mode: MissionMode,
    route: { readonly model?: string; readonly effort?: string; readonly account?: string },
    emit: (update: CodexMissionUpdate) => void,
    /**
     * Only `handOff` supplies this. It is not reachable from the renderer: the
     * start channel builds its own argument list and has no field for it, so a
     * mission cannot claim to continue another just by asking.
     */
    continuation?: MissionContinuation,
    /**
     * The teammate the mission is messaged to. Only the host supplies it,
     * from the roster it read itself: the renderer names a teammate id and
     * the host decides whether that id is anyone.
     */
    peer?: MissionPeerContext,
    /** The mission whose conversation this one continues, if any. */
    followUpOf?: string,
    /** Set by the host when a teammate is replying on their own. */
    relay?: RelayOrigin,
    /**
     * Who started this run, when it was not a person and not the relay. Only
     * `resume` supplies it; the relay derives its own from `relay` above.
     */
    startedBy?: MissionStarter,
    /**
     * The person typed one of the runtime's own slash commands (0.426,
     * runtime-commands.ts), and it is sent AS IT IS: a CLI reads a command
     * only at the very start of what it is given, and the brief would bury
     * it. Only the direct-message handler sets it, from the person's own
     * words -- never a relay, a room post or a routine, whose text is not a
     * person addressing the runtime (slash-is-composer-only.test.ts).
     */
    asCommand?: boolean,
    /**
     * A comparison's column (0.441, shared/compare.ts): the run slot it holds
     * in place of its owner's, so one teammate -- or nobody -- answers on two
     * models at once. Only the compare handler supplies it.
     */
    slot?: { readonly key: string; readonly cwd?: string; readonly instructions?: string; readonly skill?: TuneSkill; readonly sealed?: true },
    /**
     * A question ON THE SIDE of a conversation (0.461, Devin's side chats):
     * read-only, on a FORK of the session of `of` -- the conversation's
     * latest turn -- which the conversation itself never sees. The caller
     * gives it a slot of its own, so it runs while the conversation does.
     */
    side?: { readonly of: string; readonly question: number; readonly freshSession?: boolean },
    /**
     * Started again from an edited earlier message (0.498): `followUpOf` is
     * the turn before it, and the run starts cold with the conversation up to
     * there -- never that turn's session, which holds the turns set aside.
     */
    rewind?: boolean,
    /**
     * A reply on another runtime: the brief's sections the person chose to
     * leave out (0.527, handoff.ts DROPPABLE_SECTION_NAMES). Only the
     * direct-message handler supplies it, from what the composer showed.
     */
    leaveOut?: readonly string[],
    /**
     * YOUR CLOUD COMPUTER (0.761, cloud-turns.ts): the turn runs on the repository's GitHub Codespace. Only the
     * direct-message handler supplies it, from the composer's Cloud; `setUp` when the person pressed Set it up.
     */
    cloud?: { readonly setUp?: boolean },
    referenceIds?: readonly string[]
  ): Promise<CodexMissionStartResponse>
  cancel(runId: unknown): CodexMissionCancelResponse
  /**
   * The person's answer to something a run asked. False when no run of this
   * service was waiting under that id -- another host may hold it.
   */
  decide(answer: MissionApprovalAnswer): boolean
  /**
   * Pick a stopped mission back up from its last checkpoint.
   *
   * Unlike `handOff` this works on a mission that is NOT active -- that is the
   * whole case, since the usual way to end up here is the app closing
   * mid-run. So it reconciles from the ledger rather than from a live run,
   * and refuses on the same terms a handoff does when the record cannot be
   * trusted to say what happened.
   */
  resume(
    missionId: unknown,
    runtime: MissionRuntimeId,
    mode: MissionMode,
    route: { readonly model?: string; readonly effort?: string; readonly account?: string },
    emit: (update: CodexMissionUpdate) => void,
    /**
     * H6: the teammate the interrupted mission belonged to. A resume used to
     * start as nobody's: in the project folder rather than their own branch
     * or folder, with no roster brief, and outside the one-run-per-teammate
     * guard, so a second run of theirs could start beside it.
     */
    peer?: MissionPeerContext
  ): Promise<MissionHandoffResponse>
  handOff(
    runId: unknown,
    runtime: MissionRuntimeId,
    mode: MissionMode,
    route: { readonly model?: string; readonly effort?: string; readonly account?: string },
    emit: (update: CodexMissionUpdate) => void
  ): Promise<MissionHandoffResponse>
  /** What a reply on `runtime` to that mission would carry, composed as a send would and with nothing written (0.517). */
  previewSwitch(followUpOf: string, runtime: MissionRuntimeId, prompt: string, leaveOut?: readonly string[]): Promise<HandoffPreview>
  /** Whether a run of this transport is starting or live on that added account. */
  accountInUse(account: string): boolean
  /** Whether this transport owns a live run of that mission. */
  hasMission(missionId: string): boolean
  /** Includes a continued turn still being prepared, before it has a ledger row or process. */
  contextBusy(missionIds: readonly string[]): boolean
  /** The missions this transport is running right now. */
  liveMissionIds(): readonly string[]
  /**
   * The run this teammate has going here, if any.
   *
   * The busy check already asks this question internally; a teammate who may
   * be interrupted needs it asked from outside, and answering it with a
   * mission id would make the caller look the run back up.
   */
  runIdOwnedBy(teammateId: string): string | undefined
  /**
   * Show a running mission a message at its next step, without stopping it
   * (A2.10). True when the runtime took it; false for a run that cannot be
   * steered (every transport but Codex's app-server) or is no longer running.
   */
  steer(runId: string, text: string): Promise<boolean>
  interrupt(): void
  dispose(): Promise<void>
}

/**
 * The project folder a mission ran in (0.513): where a long task's file is
 * written for the run that carries it on, by the same rule `start` uses to
 * pick a continuing turn's folder.
 */
async function projectFolderOf(options: CodexMissionServiceOptions, missionId: string): Promise<string> {
  const current = options.currentFolder?.() ?? options.workspacePath
  const earlier = (await options.ledger.getMission(missionId).catch(() => undefined))?.metadata.workspaceId
  if (earlier === undefined || earlier === workspaceIdFor(current) || options.folderOf === undefined) return current
  return (await options.folderOf(earlier).catch(() => undefined)) ?? current
}

/**
 * How many normalized events may wait for the disk before the stream loop stops
 * reading (0.603). Past this the loop awaits the append in flight, and the
 * runtime's records back up into the runner's own bounded queue as they always
 * did. A thousand events is about a megabyte of envelopes.
 */
const MAX_WAITING_EVENTS = 1_000

interface CodexMissionServiceOptions {
  readonly organizationPolicy?: import('../shared/organization-policy.js').OrganizationPolicyState
  /** The folder the service was started in; `currentFolder` wins when given. */
  readonly workspacePath: string
  /** The folder the window is in NOW, for a turn that starts a conversation (0.458). */
  readonly currentFolder?: () => string
  /** A folder's path by its id, for a turn that continues a conversation from another folder (0.458). */
  readonly folderOf?: (workspaceId: string) => Promise<string | undefined>
  /** The teammate's context rebuilt for another folder: its own-branch worktree is that folder's (0.458). */
  readonly peerIn?: (peer: MissionPeerContext, folder: string) => Promise<MissionPeerContext | undefined>
  /** Test seam. Which platform's containment rules apply. */
  readonly platform?: NodeJS.Platform
  /**
   * The runtimes, for deciding whether this mission may start.
   *
   * Takes the runtime it is about to use so the host can answer from a recent
   * `ready` record instead of re-probing every other runtime — see
   * `discoverForStart`. Passing nothing is the old full sweep.
   */
  readonly discover: (runtimeId?: string, account?: string) => Promise<readonly RuntimeDiscovery[]>
  /**
   * Bring what the person did in the runtime's own terminal on a conversation
   * into its record, and say its newest turn (0.391; terminal-catch-up.ts).
   * Called before a follow-up starts, so the new turn continues from the last
   * of what was done there.
   */
  readonly catchUpTerminal?: (missionId: string) => Promise<{ readonly latestMissionId: string }>
  readonly runner: RuntimeProcessRunner
  /**
   * How to start `codex app-server`, which is the transport every Codex mode
   * runs on. Without it Codex falls back to `codex exec --json`, which works
   * but never streams -- see `startCodexAppServerRun` for the measurement.
   * Absent in tests that only exercise the exec path.
   */
  readonly appServerSpawn?: (
    executablePath: string,
    args: readonly string[],
    /** H7: the launch's own environment, merged over the host's. */
    env?: Readonly<Record<string, string>>
  ) => AppServerRunProcess
  /**
   * How to start `opencode serve`, which an OpenCode run in Approve-each rides
   * so it can stop and ask (A6.7) -- IN the teammate's folder (0.378).
   * Absent: OpenCode has no Approve-each.
   */
  /**
   * YOUR CLOUD COMPUTER (0.761, cloud-turns.ts): readies the repository's codespace for a turn sent with Cloud, says
   * which conversations ran there, and reads what a turn changed once it ends.
   */
  readonly cloud?: Pick<CloudTurns, 'of' | 'prepare' | 'settled' | 'carry'>
  /** A Claude Code turn there (0.768): the runner whose process is gh's ssh. A test's own, else cloudProcessSpawn's. */
  readonly cloudProcessRunner?: (target: { readonly codespace: string; readonly dir: string }) => RuntimeProcessRunner
  /** Codex's app server there (0.768). A test's own, else cloudAppServerSpawn's. */
  readonly cloudAppServerSpawn?: (target: { readonly codespace: string; readonly dir: string }) => (executablePath: string, args: readonly string[], env?: Readonly<Record<string, string>>) => AppServerRunProcess
  readonly opencodeServeSpawn?: (
    executablePath: string,
    args: readonly string[],
    env?: Readonly<Record<string, string>>,
    cwd?: string
  ) => AppServerRunProcess
  /**
   * How to start an Agent Client Protocol agent -- `copilot --acp`, which a
   * Copilot run in Approve-each rides so it can stop and ask (0.377) -- IN
   * the folder it is told. Absent: Copilot has no Approve-each.
   */
  /** What an ACP agent said it can do, at the start of its run (W12): kept for Settings. */
  readonly onAcpCapabilities?: (runtime: 'copilot' | 'opencode', capabilities: AcpCapabilities) => void
  /** The agents the person added (0.777, main/acp-agents.ts): an agent's command, found by the rule its Test used. */
  readonly acpAgents?: { readonly resolve: (agentId: string) => Promise<{ readonly agent: AcpAgent; readonly found: AcpLaunch } | { readonly message: string }> }
  readonly acpSpawn?: (
    executablePath: string,
    args: readonly string[],
    env?: Readonly<Record<string, string>>,
    cwd?: string
  ) => AppServerRunProcess
  readonly ledger: MissionLedger
  readonly references?: ReferenceStore
  readonly claudeContext?: Pick<ClaudeContextReader, 'remember' | 'beforeStart'>
  /**
   * A3.3: the person's check, after a turn that changed files -- in the
   * folder the run changed. Its answer goes to that run's thread; nothing
   * is sent to the teammate from here. `onStart` is told its command as it
   * starts, so the thread can say it is running (0.811).
   */
  readonly afterEdits?: (cwd: string, missionId: string, onStart?: (command: string) => void, workspaceId?: string) => Promise<EditCheckResult | undefined>
  /**
   * After the person's check said how it ended, once recorded (0.812, check-fix.ts): whether to send a failure back is
   * decided there. For a turn Locust sent back to fix it, `no-edits` when that turn changed no files, so the check was
   * not run again.
   */
  readonly onEditCheck?: (turn: CheckedTurn, result: EditCheckResult | 'no-edits') => void
  /**
   * 0.439: commit what a turn in a teammate's own worktree changed, to its
   * branch (turn-checkpoint.ts). Called for runs in a worktree only, however
   * the turn ended, before the slot is released so the next turn never
   * starts mid-commit.
   */
  readonly checkpointTurn?: (input: {
    readonly repositoryRoot: string
    readonly teammateId: string
    readonly message: CheckpointMessage
  }) => Promise<CheckpointResult>
  /**
   * Where the host writes a line nobody sees on screen. A ledger write
   * that fails mid-run used to be reported as a card that named no cause
   * and logged nowhere, so the one fact that would explain it -- EBUSY,
   * ENOSPC, a directory in the file's place -- was gone with the run.
   * Colin, 2026-09-17: "wembley never replied and his run showed an
   * error when i checked on it". Nothing on the machine could say why.
   */
  readonly note?: (label: string, detail: string) => void
  /** The teammate channel. Optional: a service without one runs missions that belong to nobody. */
  readonly workroom?: Workroom
  /** What the team remembers, briefed to every teammate mission. */
  readonly memory?: MemoryBriefing
  /**
   * What each CLI session has already been told (A2.5). Absent, every turn
   * is briefed in full, as before.
   */
  readonly briefSessions?: BriefSessions
  /**
   * Opens an attached PDF for the agent: its text and page pictures beside it
   * in the run folder's `.locust/attachments` (0.713, pdf-reading.ts).
   * Absent, a PDF is named in the message and the runtime opens it itself.
   */
  readonly readablePdf?: (folder: string, pdf: string, name: string) => Promise<ReadablePdf | undefined>
  /**
   * Called after a completed run posted messages to teammates, with what it
   * posted. Whatever this does -- a relay starting the recipient's run --
   * happens after the share is recorded and must not fail the run.
   */
  readonly onShared?: (mission: SharingMission, posted: readonly WorkroomMessage[]) => Promise<void>
  /** Called once a run is over, however it ended, after any share it made. */
  readonly onRunEnded?: (mission: EndedMission) => Promise<void>
  /** Test seam: how the working tree is looked at before and after a write-capable run. Defaults to `git status`. */
  readonly observeDisk?: (workspacePath: string) => Promise<WorkspaceSnapshot | undefined>
  /** A2.9: what each teammate's writing runs changed lately, per folder, for the overlap note. */
  readonly recentEdits?: RecentEdits
  /** Undo a turn (0.674): where the folder is kept before and after a writing run, and each turn's record of it. */
  readonly checkpoints?: Checkpoints
  readonly turnRecords?: TurnRecords
  /** Test seam: what `.cursorignore` says, if anything. See cursor-visibility.ts. */
  /** Test seam: what the Cursor CLI says about its connectors. */
  readonly readCursorIgnore?: (path: string) => Promise<string | undefined>
  /** Test seam: the change behind each changed path, read off the disk. */
  readonly observePatches?: (workspacePath: string, after: WorkspaceSnapshot, paths: readonly string[], options?: unknown, before?: WorkspaceSnapshot) => Promise<ReadonlyMap<string, ToolPatch>>
  readonly createId?: () => string
  readonly now?: () => Date
  readonly schedule?: (task: () => void) => void
  /** Events normalized and waiting for the disk before the loop stops reading (0.603); tests lower it. */
  readonly maxWaitingEvents?: number
  /**
   * Whether the workspace has Auto switched on. Asked here rather than at the
   * window's edge because every way a run can start -- a person, a relay hop,
   * a saved routine -- passes through this service, and a mode that lets a
   * runtime write anywhere on the machine should be checked on the path it
   * actually takes. Absent means off: a caller that does not wire it cannot
   * get Auto by omission.
   */
  readonly autoModeAllowed?: () => Promise<boolean>
  /** Refuse every run that is not on a free route. See `free-routes.ts`. */
  readonly freeRoutesOnly?: boolean
  /**
   * Why this teammate may not start a run now, when they have reached the
   * monthly limit the person set; undefined when they may, or have no limit.
   * Asked before EVERY start and every handoff, because every way a run
   * starts passes through here (shared/spend.ts).
   */
  readonly spendRefusal?: (teammateId: string) => Promise<string | undefined>
  /**
   * The provider a model of the person's own needs (main/own-models.ts):
   * undefined for one no longer kept. Asked only for an OpenCode route whose
   * model is `own-...`.
   */
  readonly ownProvider?: (model: string) => Promise<{ readonly id: string; readonly provider: OpenCodeProvider } | undefined>
  /**
   * Ask before every connector call: send no allow rules, so each one goes
   * to the permission host. Read at run start, never cached. The env seam
   * LOCUST_ASK_CONNECTORS=1 does the same for the drives.
   */
  readonly askConnectors?: () => Promise<boolean>
  /**
   * Puts the person's own Cursor default back after a Cursor run that named
   * a model, which Cursor saves as the default (0.431, cursor-default-model.ts).
   */
  readonly cursorDefaultModel?: CursorDefaultModel
  /** A runtime's own slash commands, as its CLI listed them this run (0.426). */
  readonly onRuntimeCommands?: (runtime: 'claude' | 'opencode', commands: readonly RuntimeCommandInfo[]) => void
  /** The person's opt-in for a runtime-kept todo list. Absent reads as off. */
  readonly keepATodoList?: () => Promise<boolean>
  /**
   * The skills a Claude Code run outside Auto is given, as plugin folders
   * built for it alone (0.679, claude-skills.ts); `dispose` removes them when
   * the run's process ends. Absent: no skills beyond Claude Code's own.
   */
  readonly claudeSkills?: (workspacePath: string, run: { readonly auto: boolean; readonly teammateId?: string; readonly skill?: TuneSkill }) => Promise<PreparedSkills>
  /**
   * The same for Codex, OpenCode, Copilot and Antigravity (0.813, run-skills.ts): what lives in Locust's profile --
   * the teammate's own skills, a tune column's one, the ones kept from GitHub -- copied for this run. Each finds the
   * folder's skills and the person's themselves.
   */
  readonly runSkills?: (run: { readonly teammateId?: string; readonly skill?: TuneSkill }) => Promise<PreparedRunSkills | undefined>
  /** Connectors a Cursor teammate can call, by name, for its briefing. */
  readonly readyConnectors?: () => Promise<string | undefined>
  /**
   * Let a Cursor run outside Auto call its connectors: merge an allow rule
   * per configured server into the workspace's `.cursor/cli.json`. Returns
   * the rules added this time and a `release` that takes them back; it is
   * called when the run's process ends. See `cursor-connector-allow.ts`.
   */
  readonly allowConnectors?: (workspace: string) => Promise<{
    readonly added: readonly string[]
    readonly release: () => Promise<void>
  }>
  /**
   * How many missions are live on the OTHER transports right now.
   *
   * The cap is one pool, not one per transport. Each of the three services
   * counted only its own, so four exec runs, four approve-each runs and four
   * Antigravity runs could all be live at once -- twelve -- while every read
   * in the app (`teammateBusy`, the sidebar count, the busy list) already
   * summed all three into one number and the refusal still said "up to 4".
   * The reporting was right and the enforcement was not.
   *
   * Defaults to zero so a service constructed alone behaves as before.
   */
  readonly liveElsewhere?: () => number
  /**
   * The connectors this machine has, by name, asked at the moment a run
   * starts rather than captured.
   *
   * Colin, 2026-09-10: "honestly just let them have access to the mcp tools
   * if the client have access to it -- it only makes sense and is way less
   * muddy." So there is nothing per-teammate here: every name becomes an
   * allow rule on every Claude Code run.
   *
   * Read live because the list is taken in the background and may not have
   * landed when the first run starts. A run that begins before it does gets
   * no rules, and its connector calls are refused with the sentence the
   * adapter writes -- which is what every build before today did, said out
   * loud, rather than a mission that waits on a health check.
   */
  readonly connectors?: () => readonly string[]
  /**
   * Locust as Claude Code's permission host. See permission-host.ts.
   *
   * Registered per run before the command is built, so the config path is
   * on the command line; released when the run ends, so a question nobody
   * answered is refused rather than left hanging and the run's token dies
   * with it. Claude Code only, and never in Auto.
   */
  readonly permissionHost?: {
    register(run: { readonly runId: string; readonly missionId: string; readonly cwd: string | null; readonly beforeApproval?: () => Promise<void> }): Promise<{
      readonly configPath: string
      readonly toolName: string
    }>
    release(runId: string): Promise<void>
  }
  /**
   * The settings file naming Locust's command guard (0.717,
   * command-guard.ts), for every Claude Code run in every mode; undefined
   * when it could not be written, and runs go without it.
   */
  readonly commandGuard?: () => Promise<string | undefined>
  /**
   * The same guard for every Copilot run (0.721, `writeCopilotCommandGuard`):
   * the plugin folder naming it, given on the print and ACP routes alike;
   * undefined when it could not be written, and runs go without it.
   */
  readonly copilotCommandGuard?: () => Promise<string | undefined>
  /** The MCP servers in the person's own Copilot settings, which Ask and Plan on Copilot start none of (0.824). */
  readonly copilotMcpServers?: () => Promise<readonly string[]>
  /**
   * The same guard for every OpenCode run and server (0.722,
   * `writeOpenCodeCommandGuard`): the plugin file's URL its config names;
   * undefined when it could not be written, and runs go without it.
   */
  readonly openCodeCommandGuard?: () => Promise<string | undefined>
  /** The directory-based V2 plugin, distinct from the V1 hook file. */
  readonly openCode2CommandGuard?: () => Promise<string | undefined>
  /**
   * Locust's git tool for an OpenCode run with no shell (0.818, commands.ts withOpenCodeGitReads); undefined when its
   * instructions file could not be written, and the run is then told only that its shell is off.
   */
  readonly openCodeGitReads?: () => Promise<GitReadsSpec | undefined>
  /**
   * Whether this runtime's next start sets itself up first (0.780, opencode-first-start.ts): OpenCode installs
   * its plugin package on its first start, a minute or more, and the start line says so instead of "Starting".
   */
  readonly setsItselfUp?: (runtime: MissionRuntimeId) => boolean
  /**
   * The same guard for a Codex run (0.720, `codexCommandGuardConfig`): the
   * config its thread is started, resumed or forked with, for this folder;
   * undefined when Codex would load some other hook too (`otherCodexHooks`),
   * and that run goes without it.
   */
  readonly codexCommandGuard?: (folder: string, codexHome?: string) => Promise<Readonly<Record<string, unknown>> | undefined>
  /**
   * What answers a Codex run that stops to ask -- Approve-each's whole point.
   * Without it that mode has nobody to ask, and every request is refused.
   */
  readonly approvals?: ApprovalChannel
  /**
   * Per-run browser MCP server host. Registers a token and config for each
   * local turn on an MCP-capable runtime and releases it when the turn ends.
   */
  readonly browserHost?: {
    registerRun(run: BrowserRunContext): Promise<BrowserMcpRegistration>
    releaseRun(runId: string): Promise<void>
  }
}

function error(
  code: 'INVALID_PROMPT' | 'RUN_ALREADY_ACTIVE' | 'CODEX_UNAVAILABLE' | 'RUNTIME_START_FAILED' | 'PERSISTENCE_FAILED' | 'RUN_NOT_ACTIVE' | 'HANDOFF_REFUSED' | 'SPEND_LIMIT_REACHED' | 'POLICY_REFUSED',
  message: string,
  busy?: 'pool'
): CodexMissionStartResponse | CodexMissionCancelResponse | MissionHandoffResponse {
  return { ok: false, error: { code, message, ...(busy === undefined ? {} : { busy }) } }
}

/** What went wrong, in the error's own words, bounded so a card stays a card. */
function reasonOf(error: unknown): string | undefined {
  const message = error instanceof Error ? error.message : typeof error === 'string' ? error : undefined
  const clean = message?.replace(/\s+/g, ' ').trim()
  return clean === undefined || clean.length === 0 ? undefined : clean.slice(0, 240)
}

function persistenceFailure(active: ActiveCodexMission, reason?: string): void {
  safelyEmit(active, {
    kind: 'persistence-error',
    runId: active.runId,
    missionId: active.missionId,
    error: {
      code: 'MISSION_PERSISTENCE_FAILED',
      // The reason rides in the sentence: the card's own wording appends
      // "Locust stopped the run rather than continue without a durable
      // record" to whatever is said here.
      message:
        reason === undefined
          ? 'The mission could not be written to the durable local ledger.'
          : `The mission could not be written to the durable local ledger: ${reason}.`
    }
  })
}

/**
 * What went wrong with the runtime's process, in its own name (R25). It said
 * "The Codex process transport ended unexpectedly" for every runtime, and
 * nothing about a program that was never started: one that was uninstalled or
 * moved, one the machine would not run, or a conversation's folder that is
 * gone -- since 0.458 every folder's conversations are one click away.
 */
export function transportSentence(active: { readonly runtime: MissionRuntimeId; readonly cwd: string }, why: unknown): string {
  const name = runtimeDisplayName(active.runtime)
  const said = why instanceof Error ? why.message : typeof why === 'string' ? why : ''
  if (!/failed to start/.test(said)) return `${name} stopped unexpectedly, before it said it had finished.`
  if (!existsSync(active.cwd)) return `${name} could not start: the folder it works in, ${active.cwd}, is not there any more.`
  if (/\(ENOENT\)/.test(said)) return `${name} could not be started: its program was not found. It may have been moved or uninstalled; Settings can install it again.`
  if (/\((EACCES|EPERM)\)/.test(said)) return `${name} could not be started: this computer did not allow its program to run.`
  return `${name} could not be started.`
}

function transportFailure(active: ActiveCodexMission, message: string): void {
  safelyEmit(active, {
    kind: 'transport-error',
    runId: active.runId,
    missionId: active.missionId,
    error: {
      code: 'RUNTIME_TRANSPORT_FAILED',
      message
    }
  })
}

async function persistTransportFailure(
  mission: ActiveCodexMission,
  ledger: MissionLedger,
  occurredAt: string,
  why?: unknown
): Promise<void> {
  const message = transportSentence(mission, why)
  try {
    await ledger.appendHostFailure(mission.missionId, {
      code: 'runtime-transport-failed',
      message,
      occurredAt
    })
    transportFailure(mission, message)
  } catch {
    persistenceFailure(mission)
  }
}

function safelyEmit(active: ActiveCodexMission, update: CodexMissionUpdate): void {
  try {
    active.emit(update)
  } catch {
    // A closed renderer must not destabilize or orphan the host process.
  }
}

async function persistAndEmit(
  active: ActiveCodexMission,
  ledger: MissionLedger,
  events: ReturnType<CodexEventNormalizer['accept']>
): Promise<void> {
  await persistEventUpdates(ledger, active.runId, active.missionId, events, (durable) => {
    // Tracked only once durable, so a share reads what the ledger holds.
    active.transcript.track(durable)
    for (const event of durable) {
      if (event.sequence > active.lastSequence) active.lastSequence = event.sequence
      active.persisted.push(event)
    }
  }, (update) => safelyEmit(active, update))
}

function validPrompt(value: unknown): value is string {
  return typeof value === 'string'
    && value.trim().length > 0
    && value.length <= MAX_PROMPT_LENGTH
    && !value.includes('\0')
}

/**
 * The runtime's own handle for a finished mission's conversation, from its own
 * records. `run.started` states it at the beginning and the terminal events
 * repeat it; the last one that carries it wins, since a resumed session can
 * report a new id.
 */
export function runtimeThreadIdOf(mission: RecoveredMission): string | undefined {
  let held: string | undefined
  for (const event of mission.events) {
    const payload = event.payload as { readonly runtimeThreadId?: unknown; readonly sessionEnded?: unknown }
    // M7: a failure that ended the session leaves nothing to resume -- the
    // next turn starts fresh, as its card told the person it would.
    if (event.type === 'run.failed' && payload.sessionEnded === true) {
      held = undefined
      continue
    }
    if (typeof payload.runtimeThreadId === 'string' && payload.runtimeThreadId.length > 0) {
      held = payload.runtimeThreadId
    }
  }
  return held
}

/**
 * The session a reply to `prior` resumes on `runtime`, if any: its own, or,
 * for a turn stopped before it named one, the one it resumed (0.496).
 *
 * None when a failure ended the session -- the fallback used to revive it --
 * and none for a conversation Antigravity's APP held (0.541, Colin's Boss):
 * that id is the app's, and Antigravity CLI answered "trajectory not found".
 * The reply then starts fresh with the conversation so far.
 */
export function resumableThreadOf(prior: RecoveredMission, runtime: string): string | undefined {
  if (runtime === 'antigravity' && prior.metadata.resolvedRouteId === 'antigravity:hub') return undefined
  const ended = prior.events.some((event) => {
    if (event.type !== 'run.failed') return false
    const payload = event.payload as { readonly sessionEnded?: unknown; readonly message?: unknown }
    // 0.540 recorded Antigravity's "trajectory not found" without saying the session had ended.
    return payload.sessionEnded === true || (runtime === 'antigravity' && typeof payload.message === 'string' && /trajectory not found/i.test(payload.message))
  })
  if (ended) return undefined
  return runtimeThreadIdOf(prior) ?? prior.metadata.continuesFrom?.runtimeThreadId
}

/**
 * The connectors a Claude run is given allow rules for (0.545). None in Ask or
 * Plan (read-only): there every connector call asks, as Claude Code's own plan
 * mode does -- measured with a test server, 2026-10-03. None when the person
 * asked to be asked about every one. Otherwise the named ones.
 */
/**
 * What OpenCode is told when Ask or Plan refuses something nobody was asked about (0.822): the mode, in the words the
 * person sees, so the model's reply says why. The refusal used to read "The person declined this in Locust", and on
 * the 0.821 drive the model told a person who had seen no card "you declined the permission request".
 */
export function openCodeReadOnlyRefusal(mode: MissionMode): string {
  const named = mode === 'plan' ? 'Plan mode, which reads and plans' : 'Ask mode, which reads'
  return `Locust refused this without asking anyone: this teammate is in ${named} and changes nothing, so nothing was run.`
}

/** Whether a run loads the person's connectors: only for a teammate that has one ticked (0.798). */
export function connectorsOn(ticked: readonly string[] | undefined): boolean {
  return ticked !== undefined && ticked.length > 0
}

export function claudeConnectorRules(sandbox: string, askEvery: boolean, named: readonly string[]): { readonly connectors?: readonly string[] } {
  if (askEvery || sandbox === 'read-only' || named.length === 0) return {}
  return { connectors: named }
}

function validRunId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= 128
    && !value.includes('\0')
}

/** How many turns before the one handed over a switch brief recalls (A2.11). */
const EARLIER_TURNS = 4

const CURSOR_READ_ONLY_REFUSAL = 'Cursor Agent cannot be held read-only on this system: its sandbox needs macOS or Linux, and plan mode alone does not stop it editing files. Choose "Edit" if it may change this workspace, or run this on Codex CLI or Claude Code.'
const AUTO_OFF_REFUSAL = 'Auto mode is switched off for this workspace. Turn it on in Settings to let a run work outside the workspace folder. Nothing was recorded.'
const unreadableRuntimeRefusal = (runtime: MissionRuntimeId): string =>
  `${runtimeDisplayName(runtime)} is installed and signed in, but Locust cannot read its event stream yet. Choose Codex CLI or Claude Code for this mission.`
/*
 * WHICH KIND OF NOT READY (0.495). One sentence -- "Install or sign in" --
 * covered a runtime that is not installed, one signed out, and one whose own
 * service did not answer. The last sent people to sign in again when Cursor's
 * servers were resetting connections (Colin, 2026-09-30: "is this a cursor
 * problem or an us problem"). Discovery already knows which.
 */
const notReadyRefusal = (runtime: MissionRuntimeId, found?: RuntimeDiscovery): string => {
  const name = runtimeDisplayName(runtime)
  if (found === undefined || found.availability !== 'available' || found.executable === undefined) return `${name} is not installed on this machine. Install it, then retry discovery.`
  if (found.readiness === 'authentication-required') return `${name} is signed out. Sign in to it, then retry discovery.`
  if (found.readiness === 'unhealthy') return `${name} could not be reached just now: it did not answer when Locust checked on it, which usually means its own service is down. Try again in a minute, or pick another model.`
  return `${name} is not ready. Install or sign in to it, then retry discovery.`
}

/** A turn as the fix loop is told it (0.812, check-fix.ts), when its check has said how it ended. */
function checkedTurnOf(mission: ActiveCodexMission): CheckedTurn {
  return {
    missionId: mission.missionId,
    workspaceId: mission.workspaceId,
    ...(mission.peer === undefined ? {} : { teammateId: mission.peer.self.teammateId }),
    completed: mission.transcript.completed,
    apart: mission.slot !== undefined || mission.relay !== undefined,
    ...(mission.startedBy === undefined ? {} : { startedBy: mission.startedBy })
  }
}

export function createCodexMissionService(options: CodexMissionServiceOptions): CodexMissionService {
  // Resolved here, not inside `start`: that scope declares its own `process`
  // for the child, which shadows Node's global and is in its temporal dead
  // zone at the point this is needed.
  const hostPlatform: NodeJS.Platform = options.platform ?? process.platform

  if (!isAbsolute(options.workspacePath)) {
    throw new Error('Codex mission workspace must be an absolute path')
  }

  const createId = options.createId ?? randomUUID
  const now = options.now ?? (() => new Date())
  const schedule = options.schedule ?? ((task: () => void) => {
    setImmediate(task)
  })
  /** Live missions by runId. */
  const active = new Map<string, ActiveCodexMission>()
  /** Owners with a start in flight, between the guard and activation. */
  const starting = new Set<string>()
  const contextStarting = new Map<string, string>()
  let freshClaims = 0
  /** Each live mission's consume loop, so a handoff can wait for ITS run alone. */
  const settling = new Map<string, Promise<void>>()
  let lifecycleVersion = 0
  let disposed = false
  const interruptedMissionIds = new Set<string>()
  const startOperations = new Set<Promise<void>>()
  const consumeOperations = new Set<Promise<void>>()
  // A comparison's column holds its own slot (0.441), teammate or not; everything else, its owner's.
  const ownerKeyOf = (peer: MissionPeerContext | undefined, slot?: string): string => slot ?? peer?.self.teammateId ?? NOBODY
  const peerExchange: PeerExchange | undefined =
    options.workroom === undefined
      ? undefined
      : createPeerExchange({
          workroom: options.workroom,
          ledger: options.ledger,
          ...(options.memory === undefined ? {} : { memory: options.memory }),
          ...(options.keepATodoList === undefined ? {} : { keepATodoList: options.keepATodoList }),
          ...(options.readyConnectors === undefined ? {} : { readyConnectors: options.readyConnectors })
        })

  /*
   * Who waits for a run to leave `active` (0.812): what follows its check is the same teammate's next turn, which
   * cannot start while this one still holds the teammate -- and a turn in its own copy holds it through a git commit
   * that a quick check can outlast.
   */
  const clearedWaiters = new Map<string, (() => void)[]>()
  const whenCleared = (mission: ActiveCodexMission): Promise<void> => {
    if (active.get(mission.runId) !== mission) return Promise.resolve()
    return new Promise((resolve) => {
      clearedWaiters.set(mission.runId, [...(clearedWaiters.get(mission.runId) ?? []), resolve])
      // Never held for ever: a run that somehow stays is answered as gone after two minutes.
      setTimeout(resolve, 120_000).unref?.()
    })
  }

  const clearActive = (candidate: ActiveCodexMission): void => {
    if (active.get(candidate.runId) === candidate) active.delete(candidate.runId)
    for (const resolve of clearedWaiters.get(candidate.runId) ?? []) resolve()
    clearedWaiters.delete(candidate.runId)
    // The run is over, so nobody can answer a permission it was still asking:
    // refuse what is waiting, forget its token, remove its config.
    void options.permissionHost?.release(candidate.runId).catch(() => undefined)
    void options.browserHost?.releaseRun(candidate.runId).catch(() => undefined)
    void options.references?.release(candidate.runId).catch(() => undefined)
    options.approvals?.release(candidate.runId)
    // Whatever ended this run, anyone waiting on it is told. Fire and forget:
    // a meeting's bookkeeping must never hold a slot open.
    if (options.onRunEnded !== undefined) {
      void options
        .onRunEnded({ missionId: candidate.missionId, peer: candidate.peer, relay: candidate.relay })
        .catch(() => undefined)
    }
  }

  /**
   * Say why a read was refused, AT THE MOMENT IT IS REFUSED.
   *
   * Cursor obeys `.cursorignore` and its tools answer "permission denied"
   * with no reason, so the model invents one and the person chases the
   * invention. That cost an evening on 2026-09-12.
   *
   * 0.87.0 fixed it by warning at the START of every Cursor run in a hidden
   * folder. Which is TRUE and was still wrong: Colin's teammates in that
   * folder do web research and never open a file, so the warning was correct,
   * inapplicable and unavoidable -- an amber line above every exchange,
   * forever. 0.91.2 reduced it to once per folder per session; he restarted
   * the app all night and asked again, which is the right question: "is it
   * necessary to have the error".
   *
   * Not preemptively. A rule that hides a folder only matters when something
   * is actually refused, and Cursor says so precisely -- the raw record of a
   * failed tool carries `errorMessage: "Permission denied"`, unredacted. So
   * the explanation waits for that, arrives beside the failure it explains,
   * and a run that never opens a file never hears about it.
   *
   * Once per mission. The second denial has the same cause as the first.
   */
  const explainADeniedRead = async (
    mission: ActiveCodexMission,
    events: readonly NormalizedRuntimeEvent[]
  ): Promise<void> => {
    if (mission.runtime !== 'cursor' || mission.saidWhyAReadFailed) return
    const denied = events.some(
      (event) =>
        event.type === 'tool.failed'
        && /permission denied/i.test(JSON.stringify(event.payload.evidence?.raw ?? ''))
    )
    if (!denied) return
    mission.saidWhyAReadFailed = true
    try {
      const sentence = await cursorCannotSee(mission.cwd, options.readCursorIgnore)
      if (sentence === undefined) return
      // Shown, never recorded: see the note where this used to be raised.
      safelyEmit(mission, {
        kind: 'event',
        runId: mission.runId,
        missionId: mission.missionId,
        event: cursorIgnoreNotice({
          runId: mission.runId,
          missionId: mission.missionId,
          sourceAdapter: mission.runtime,
          nextSequence: -1,
          at: now().toISOString(),
          sentence
        })
      })
    } catch {
      // A rule file that cannot be read explains nothing.
    }
  }


  const consume = async (mission: ActiveCodexMission): Promise<void> => {
    let persistenceFailed = false
    let persistenceReason: string | undefined
    const maxWaiting = options.maxWaitingEvents ?? MAX_WAITING_EVENTS
    let ledgerFailed = false
    const batches = createStreamedEventBatcher(async (events) => {
      await persistAndEmit(mission, options.ledger, events)
      await mission.deliverMessages(events)
      // Best effort, after the durable write; this is not a ledger failure.
      await explainADeniedRead(mission, events).catch(() => undefined)
    }, (error) => {
      persistenceReason = reasonOf(error)
      options.note?.('ledger-write-failed', `${mission.missionId}: ${persistenceReason ?? 'no message'}`)
      ledgerFailed = true
      persistenceFailed = true
      mission.controller.abort()
    })
    mission.flushEvents = () => batches.flush()
    try {
      for await (const record of mission.process.records) {
        try {
          // Take everything already buffered along with this record, so the
          // runner's bounded queue does not fill while this loop works -- a
          // full queue ends the run as an output-limit breach, so a verbose
          // mission would be killed for being verbose.
          // Defensive: a stream from a source that does not implement draining
          // must degrade to one record at a time, not throw -- a TypeError
          // here would be caught below and reported as a persistence failure,
          // which is a misleading thing to tell a user about a working ledger.
          const buffered = typeof mission.process.records.drainAvailable === 'function'
            ? mission.process.records.drainAvailable()
            : []
          const batch = [record, ...buffered]
          /*
           * THREE different failures used to share one catch, and the card
           * blamed the ledger for all of them. The adapter throwing on a
           * record it cannot read, the ledger refusing the append, and the
           * denied-read explainer failing are told apart here; only the
           * middle one is a persistence failure, and it now says why.
           */
          let events: ReturnType<CodexEventNormalizer['accept']>
          try {
            events = batch.flatMap((entry) => [...mission.normalizer.accept(entry)])
          } catch (error) {
            const reason = reasonOf(error) ?? 'the adapter threw'
            options.note?.('adapter-failed', `${mission.missionId} ${mission.runtime}: ${reason}`)
            persistenceFailed = true
            persistenceReason = `the ${mission.runtime} adapter could not read a record from the runtime (${reason})`
            mission.controller.abort()
            break
          }
          // The first event of the run dates the end of the start (0.602).
          if (events.length > 0) mission.startTiming.firstEventAt ??= Date.now()
          batches.push(events)
          if (batches.pendingCount >= maxWaiting) await batches.flush()
          if (ledgerFailed) break
        } catch (error) {
          // Anything else that escapes the guarded steps above: still a
          // stop, still with whatever reason the error carries.
          persistenceReason = reasonOf(error)
          persistenceFailed = true
          mission.controller.abort()
          break
        }
      }
    } catch {
      // The completion receipt below determines whether this was a bounded-output
      // stop or an unrecoverable transport failure.
    }
    // What was still being written, or waiting to be, lands before the run's
    // end is read -- including what a failing adapter left behind it.
    await batches.flush()

    if (persistenceFailed) {
      try {
        await mission.process.completion
      } catch {
        // The durable-write failure is the authoritative outcome.
      } finally {
        // Emit only after the aborted process has fully terminated so the
        // renderer's failed state and the service's availability agree: a
        // retry submitted after this update never hits RUN_ALREADY_ACTIVE.
        persistenceFailure(mission, persistenceReason)
        clearActive(mission)
      }
      return
    }

    let completion
    try {
      completion = await mission.process.completion
    } catch (error) {
      mission.settled = true
      await persistTransportFailure(mission, options.ledger, now().toISOString(), error)
      clearActive(mission)
      return
    }
    // From here the process is gone. Everything below -- the terminal events,
    // the disk observation, the share, the relay -- is bookkeeping, and a run
    // started during any of it is not running alongside this one.
    mission.settled = true

    let terminalEvents: ReturnType<CodexEventNormalizer['finish']>
    try {
      terminalEvents = mission.normalizer.finish(completion)
    } catch {
      await persistTransportFailure(mission, options.ledger, now().toISOString())
      clearActive(mission)
      return
    }

    try {
      await persistAndEmit(mission, options.ledger, terminalEvents)
    } catch {
      persistenceFailure(mission)
      clearActive(mission)
      return
    }

    // What the run changed that it never said. After the terminal events so
    // the record's own account comes first and the observation reads as what
    // it is -- the host looking at the tree afterwards. Best effort: an
    // observation that cannot be made or stored costs the run nothing.
    // The runtime's own events always stand -- those name their own tools.
    // What follows is the HOST's reading of the folder, and when another run
    // was writing in it the reading names nobody: it is reported as the
    // folder's change rather than attached to this teammate as files.
    /*
     * A2.17: what the host itself saw this run change, for the messages it
     * sends. Only from a folder this run had to itself -- in a shared one the
     * reading names nobody, so it proves nothing about this teammate -- and
     * only when the host looked at all.
     */
    let observed: readonly string[] | undefined
    if (mission.diskBefore !== undefined) {
      try {
        const diskAfter = await (options.observeDisk ?? snapshotWorkspace)(mission.cwd)
        if (diskAfter !== undefined && mission.sharedTree) {
          const changed = changedPaths(mission.diskBefore, diskAfter)
          if (changed.length > 0) {
            await persistAndEmit(
              mission,
              options.ledger,
              [
                sharedTreeNotice({
                  runId: mission.runId,
                  missionId: mission.missionId,
                  sourceAdapter: mission.runtime,
                  nextSequence: mission.lastSequence + 1,
                  at: now().toISOString(),
                  paths: changed
                })
              ] as ReturnType<CodexEventNormalizer['accept']>
            )
          }
        } else if (diskAfter !== undefined) {
          // Every path that changed, with the change read off the disk. A
          // path the runtime named keeps its own row and gets the patch
          // attached; one it never named gets a row of its own.
          const changed = changedPaths(mission.diskBefore, diskAfter)
          observed = changed
          if (mission.peer !== undefined) {
            options.recentEdits?.record({ folder: mission.cwd, teammateId: mission.peer.self.teammateId, name: mission.peer.self.name, paths: changed, at: now() })
          }
          const unreported = new Set(unreportedPaths(changed, mission.persisted))
          const patches = changed.length === 0
            ? new Map<string, ToolPatch>()
            : await (options.observePatches ?? observedPatches)(mission.cwd, diskAfter, changed,
              // A large file the snapshot could not keep is compared against the turn's own Undo copy (0.695).
              mission.checkpointBefore?.ok === true && options.checkpoints !== undefined
                ? { beforeText: (path) => options.checkpoints!.fileAt(mission.cwd, (mission.checkpointBefore as { readonly commit: string }).commit, path) }
                : {},
              mission.diskBefore)
          // Every changed path is worth its event (0.597): an unnamed one gets a row, a named one its
          // patch -- or, when its text could not be read, the word that it changed on disk, which the
          // thread lays on the runtime's row in place of "did not report the change".
          const worth = changed
          if (worth.length > 0) {
            await persistAndEmit(
              mission,
              options.ledger,
              observedEditEvents({
                runId: mission.runId,
                missionId: mission.missionId,
                sourceAdapter: mission.runtime,
                nextSequence: mission.lastSequence + 1,
                paths: worth,
                at: now().toISOString(),
                patches,
                reported: new Set(changed.filter((path) => !unreported.has(path)))
              }) as ReturnType<CodexEventNormalizer['accept']>
            )
          }
        }
      } catch {
        // The receipt stands on the runtime's own events.
      }
    }

    // Undo a turn (0.674): the folder as the run left it, kept, and the turn's way back recorded. Best effort: a
    // record that cannot be made costs the run nothing, and the turn simply offers no undo.
    if (mission.checkpointBefore?.ok === true && options.checkpoints !== undefined && options.turnRecords !== undefined) {
      try {
        const after = await options.checkpoints.take(mission.cwd)
        if (after.ok) {
          await options.checkpoints.hold(mission.cwd, `${mission.runId}-before`, mission.checkpointBefore.commit)
          await options.checkpoints.hold(mission.cwd, `${mission.runId}-after`, after.commit)
          await options.turnRecords.put(mission.runId, {
            workspace: mission.cwd,
            before: mission.checkpointBefore.commit,
            after: after.commit,
            at: now().toISOString(),
            notKept: [...new Set([...mission.checkpointBefore.notKept, ...after.notKept])],
            ...(mission.sharedTree ? { shared: true as const } : {})
          })
        }
      } catch {
        // No way back for this turn; the turn itself stands.
      }
    }

    // Where the seconds before "started" went (0.602): one host note after the run's own events.
    const timingNote = startTimingNote({
      runId: mission.runId,
      missionId: mission.missionId,
      sourceAdapter: mission.runtime,
      nextSequence: mission.lastSequence + 1,
      at: now().toISOString(),
      runtimeName: runtimeDisplayName(mission.runtime),
      timing: mission.startTiming
    })
    if (timingNote !== undefined) {
      try {
        await persistAndEmit(mission, options.ledger, [timingNote] as ReturnType<CodexEventNormalizer['accept']>)
      } catch {
        // The receipt stands on the runtime's own events.
      }
    }

    // Share only from a run that finished on its own terms, and before the
    // slot is released: a handoff waits on this loop, so it never reconciles
    // a mission whose findings are still being posted.
    if (mission.transcript.completed && mission.peer !== undefined && peerExchange !== undefined) {
      const text = mission.transcript.latestFinal
      if (text !== undefined) {
        const posted = await peerExchange.share(
          { runId: mission.runId, missionId: mission.missionId, peer: mission.peer, text, ...(observed === undefined ? {} : { observed }) },
          (update) => safelyEmit(mission, update)
        )
        if (posted.length > 0 && options.onShared !== undefined) {
          try {
            await options.onShared(
              {
                runId: mission.runId,
                missionId: mission.missionId,
                runtime: mission.runtime,
                sandbox: mission.sandbox,
                model: mission.model,
                peer: mission.peer,
                relay: mission.relay,
                ...(mission.startedBy === undefined ? {} : { startedBy: mission.startedBy })
              },
              posted
            )
          } catch {
            // The share stands and is recorded; a relay that could not start
            // is the relay's own notice to give, not a failure of this run.
          }
        }
      }
    }
    // A3.3: the person's check, only after a turn that may write and did.
    // Not awaited: a check can take minutes, and the slot is not its to hold.
    if (observed !== undefined && observed.length > 0 && mission.sandbox !== 'read-only' && options.afterEdits !== undefined) {
      // Said as it starts (0.811), so the thread can show it running; one that then throws is ended in words.
      let started: string | undefined
      void options.afterEdits(mission.cwd, mission.missionId, (command) => {
        started = command
        safelyEmit(mission, { kind: 'edit-check-started', runId: mission.runId, missionId: mission.missionId, command })
      }, mission.workspaceId)
        .catch(() => {
          if (started === undefined) return undefined
          return { command: started, outcome: 'could-not-run' as const, newLines: ['The check stopped before it said how it ended.'], unchanged: false, first: false }
        })
        .then(async (result) => {
          if (result === undefined) return
          // Its own fields only: the whole output kept for a fix (`lines`, `before`) is not the window's.
          safelyEmit(mission, { kind: 'edit-check', runId: mission.runId, missionId: mission.missionId, command: result.command, outcome: result.outcome, newLines: result.newLines, unchanged: result.unchanged, first: result.first })
          // Onto the mission it ran after, so a reopened conversation still
          // has the card and its Send button. A failed write loses only that.
          await options.ledger
            .appendEditCheck(mission.missionId, boundedEditCheck({ ...result, occurredAt: now().toISOString() }))
            .catch((error: unknown) => options.note?.('edit-check-write-failed', `${mission.missionId}: ${error instanceof Error ? error.message : 'no message'}`))
          // After it is on screen and in the record, so a fix that follows is never ahead of the failure it fixes; and
          // after the turn has let go of its teammate, since a fix is that teammate's next turn.
          if (options.onEditCheck === undefined) return
          await whenCleared(mission)
          try {
            options.onEditCheck(checkedTurnOf(mission), result)
          } catch {
            // What follows a check is its own concern; the check stands.
          }
        })
        .catch(() => undefined)
    } else if (mission.startedBy?.kind === 'check-fix' && observed !== undefined && observed.length === 0) {
      // A fix that changed nothing leaves the check where it was (0.812): said under it, rather than ending in silence.
      const said = options.onEditCheck
      if (said !== undefined) {
        void whenCleared(mission).then(() => said(checkedTurnOf(mission), 'no-edits')).catch(() => undefined)
      }
    }
    // 0.439: a turn in the teammate's own worktree is committed to its branch,
    // however it ended, with a receipt in its stream. Best effort: a commit
    // that cannot be made says so, and the files stay in the tree.
    // Only the teammate's OWN run commits to its branch (0.532). A run in a slot of its
    // own -- a comparison's column, a judge, a side question -- carried the teammate's
    // context and committed its branch with the comparison's ask as the message, sweeping
    // in whatever a direct run was half-way through there at the time.
    if (mission.peer?.repositoryRoot !== undefined && mission.slot === undefined && options.checkpointTurn !== undefined) {
      // From every event the run WROTE, not only the batch `finish` returned (turnOutcomeOf).
      const outcome: TurnOutcome = turnOutcomeOf(mission.persisted)
      const teammate = { teammateId: mission.peer.self.teammateId, name: mission.peer.self.name }
      const result = await options
        .checkpointTurn({
          repositoryRoot: mission.peer.repositoryRoot,
          teammateId: teammate.teammateId,
          message: checkpointMessage({ prompt: mission.prompt, answer: mission.transcript.latestFinal, teammate, missionId: mission.missionId, runtime: mission.runtime, model: mission.model, outcome })
        })
        .catch((error: unknown) => ({ kind: 'failed' as const, message: error instanceof Error ? error.message : 'git did not say why.' }))
      const sentence = checkpointSentence(result)
      if (sentence !== undefined) {
        await persistAndEmit(
          mission,
          options.ledger,
          [checkpointNotice({ runId: mission.runId, missionId: mission.missionId, sourceAdapter: mission.runtime, nextSequence: mission.lastSequence + 1, at: now().toISOString(), sentence, failed: result.kind === 'failed' })] as ReturnType<CodexEventNormalizer['accept']>
        ).catch(() => undefined)
      }
    }
    clearActive(mission)
  }

  /**
   * A2.11: the conversation's turns before `prior`, oldest first, walked back
   * through each mission's `continuesFrom`. Best effort: a turn the ledger
   * cannot read ends the walk, and the brief goes without what is past it.
   */
  const earlierTurnsOf = async (prior: { readonly metadata: { readonly continuesFrom?: { readonly missionId: string } } }): Promise<readonly EarlierTurn[]> => {
    const turns: EarlierTurn[] = []
    let from = prior.metadata.continuesFrom?.missionId
    const seen = new Set<string>()
    while (from !== undefined && turns.length < EARLIER_TURNS && !seen.has(from)) {
      seen.add(from)
      const earlier = await options.ledger.getMission(from).catch(() => undefined)
      if (earlier === undefined) break
      const transcript = createTranscriptTracker()
      transcript.track(earlier.events)
      turns.unshift({ asked: earlier.metadata.prompt, answered: transcript.latestFinal })
      from = earlier.metadata.continuesFrom?.missionId
    }
    return turns
  }

  const scheduleConsume = (mission: ActiveCodexMission): void => {
    let resolveOperation!: () => void
    const operation = new Promise<void>((resolve) => {
      resolveOperation = resolve
    })
    consumeOperations.add(operation)
    settling.set(mission.runId, operation)
    schedule(() => {
      void consume(mission).finally(() => {
        consumeOperations.delete(operation)
        settling.delete(mission.runId)
        resolveOperation()
      })
    })
  }

  /**
   * A personal limit check that throws keeps its existing fail-open behavior
   * and is logged. Managed checks return an explicit refusal when receipts
   * cannot be read (organization-spend.ts), rather than throwing here.
   */
  async function spendRefusalFor(teammateId = ''): Promise<string | undefined> {
    if (options.spendRefusal === undefined) return undefined
    try {
      return await options.spendRefusal(teammateId)
    } catch (cause) {
      options.note?.('spend-limit', `could not check ${teammateId}'s monthly limit: ${cause instanceof Error ? cause.message : String(cause)}`)
      return undefined
    }
  }

  /**
   * M9 (the code review): why `runtime` would refuse this run whatever the
   * run did -- asked before a handoff stops anything. These are start()'s own
   * checks that do not depend on the run: a paid route in a free window, Auto
   * switched off, Cursor read-only where its sandbox cannot hold, a runtime
   * that is not ready, one whose events Locust cannot read. The handoff used
   * to learn them from the new start, after the run was already stopped, so
   * the work in flight was lost and nothing ran on either side.
   */
  async function targetRefusal(
    runtime: MissionRuntimeId,
    mode: MissionMode,
    route: { readonly model?: string; readonly effort?: string; readonly account?: string },
    /** The teammate the run belongs to, whose monthly limit applies to the run it hands to. */
    teammateId?: string
  ): Promise<string | undefined> {
    const managed = policyTurnRefusal(options.organizationPolicy, { runtime, mode, model: route.model, account: route.account })
    if (managed !== undefined) return managed
    const chosenModel = route.model === undefined || route.model === 'account-default' ? undefined : route.model
    if (options.freeRoutesOnly === true && !isFreeRoute(runtime, chosenModel)) return FREE_ONLY_REFUSAL
    const overLimit = await spendRefusalFor(teammateId)
    if (overLimit !== undefined) return overLimit
    if (mode === 'auto' && !(await (options.autoModeAllowed ?? (async () => false))())) return AUTO_OFF_REFUSAL
    const readOnly = mode !== 'auto' && mode !== 'accept-edits' && mode !== 'approve-each'
    if (runtime === 'cursor' && readOnly && !cursorCanEnforceReadOnly(hostPlatform)) return CURSOR_READ_ONLY_REFUSAL
    if (runtime === 'acp') {
      const found = await acpAgentFor(mode, chosenModel)
      return 'refusal' in found ? found.refusal : undefined
    }
    let runtimes: readonly RuntimeDiscovery[]
    try {
      runtimes = await options.discover(runtime, ...(route.account === undefined ? [] : [route.account]))
    } catch {
      return `${runtimeDisplayName(runtime)} readiness could not be verified. Check the local runtime and try again.`
    }
    const chosen = runtimes.find((entry) => entry.id === runtime)
    if (runtime === 'cursor') {
      const picked = cursorChoiceFor(cursorModelsFrom(runtimes), chosenModel, route.effort)
      const managedVariant = policyTurnRefusal(options.organizationPolicy, { runtime, mode, model: picked.model })
      if (managedVariant !== undefined) return managedVariant
    }
    if (chosen?.availability !== 'available' || chosen.readiness !== 'ready' || chosen.executable === undefined) {
      return notReadyRefusal(runtime, chosen)
    }
    if (!hostReadsEventsOf(runtime)) return unreadableRuntimeRefusal(runtime)
    return undefined
  }

  /**
   * An agent the person added (0.777): runs only in Approve each, needs the protocol's transport, and is found by its
   * own command line. The route's model names it, with the model picked on it after `::` (shared/acp-agents.ts).
   */
  const acpAgentFor = async (mode: MissionMode, routeModel: string | undefined): Promise<{ readonly agent: AcpAgent; readonly found: AcpLaunch; readonly model?: string } | { readonly refusal: string }> => {
    const named = parsedAcpRouteModel(routeModel)
    if (named === undefined || options.acpAgents === undefined || options.acpSpawn === undefined) {
      return { refusal: 'Pick one of your own agents in the model picker, or add one in Settings > AI agents > Your own agents.' }
    }
    const resolved = await options.acpAgents.resolve(named.agentId)
    if ('message' in resolved) return { refusal: resolved.message }
    if (mode !== 'approve-each') {
      return { refusal: `${resolved.agent.name} runs in Approve each, where every action it asks to take is a card. Pick Approve each and send again.` }
    }
    return { agent: resolved.agent, found: resolved.found, ...(named.model === undefined ? {} : { model: named.model }) }
  }

  const service: CodexMissionService = {
    async start(
      prompt: unknown,
      runtime: MissionRuntimeId,
      mode: MissionMode,
      route: { readonly model?: string; readonly effort?: string; readonly account?: string },
      emit: (update: CodexMissionUpdate) => void,
      continuation?: MissionContinuation,
      peer?: MissionPeerContext,
      followUpOf?: string,
      relay?: RelayOrigin,
      startedBy?: MissionStarter,
      asCommand?: boolean,
      slot?: { readonly key: string; readonly cwd?: string; readonly instructions?: string; readonly skill?: TuneSkill; readonly sealed?: true },
      side?: { readonly of: string; readonly question: number; readonly freshSession?: boolean },
      rewind?: boolean,
      leaveOut?: readonly string[],
      cloud?: { readonly setUp?: boolean },
      referenceIds?: readonly string[]
    ): Promise<CodexMissionStartResponse> {
      // All callers, including routines, relays, compares, resume and handoffs, cross this gate before I/O.
      const managed = policyTurnRefusal(options.organizationPolicy, { runtime, mode, model: route.model, account: route.account, cloud: cloud !== undefined })
      if (managed !== undefined) return error('POLICY_REFUSED', managed) as CodexMissionStartResponse
      // The start's phases (0.602): said to the window as they begin, and timed for the note the run's end writes.
      if (route.account !== undefined && cloud !== undefined) return error('RUNTIME_START_FAILED', 'Added accounts run on this computer. Choose This PC\'s sign-in for a cloud turn.') as CodexMissionStartResponse
      const sentAt = Date.now()
      const phaseAt: Partial<Record<StartPhase, number>> = {}
      const say = (phase: StartPhase): void => {
        const at = Date.now()
        phaseAt[phase] = at
        // Timed as the start it is; said as what the person is waiting for (0.780). From its first phase (0.813): the
        // look for OpenCode waits on the same first-start install, and the clean-machine drive read "Looking for
        // OpenCode" for the whole minute of it.
        const shown: StartPhase = (phase === 'starting-runtime' || phase === 'looking') && options.setsItselfUp?.(runtime) === true ? 'runtime-first-setup' : phase
        emit({ kind: 'start-phase', runtime, ...(peer === undefined ? {} : { teammateId: peer.self.teammateId }), phase: shown, at: new Date(at).toISOString() })
      }
      // `account-default` is the shell's word for "send no --model", not a
      // model id. Passing it through would make the CLI look for a model that
      // does not exist.
      const chosenModel =
        route.model === undefined || route.model === 'account-default' ? undefined : route.model
      // Before anything is recorded or spawned: a drive's window spends
      // nothing, whichever way the run was asked for (free-routes.ts).
      if (options.freeRoutesOnly === true && !isFreeRoute(runtime, chosenModel)) {
        return error('RUNTIME_START_FAILED', FREE_ONLY_REFUSAL) as CodexMissionStartResponse
      }
      /*
       * THE RUN'S OWN FOLDER (0.458, docs/PLAN-2026-09-29-FOLDERS-LIKE-CLAUDE-CODE.md).
       *
       * A conversation belongs to the folder it began in, and the window can
       * now be in another one without a restart. So a turn that continues a
       * conversation -- a follow-up, a route switch, a relayed reply -- runs
       * where that conversation ran, never where the window happens to be.
       * One whose folder is not known, or is gone, is refused rather than
       * run somewhere it never was.
       */
      const continues = continuation?.missionId ?? followUpOf ?? relay?.rootMissionId ?? side?.of
      const current = options.currentFolder?.() ?? options.workspacePath
      let runFolder = current
      if (continues !== undefined && options.folderOf !== undefined) {
        const earlierFolder = (await options.ledger.getMission(continues).catch(() => undefined))?.metadata.workspaceId
        if (earlierFolder !== undefined && earlierFolder !== workspaceIdFor(current)) {
          const path = await options.folderOf(earlierFolder).catch(() => undefined)
          if (path === undefined) {
            return error('RUNTIME_START_FAILED', 'Locust does not know which folder this conversation ran in, so it will not continue it somewhere else. Start a new conversation instead.') as CodexMissionStartResponse
          }
          if (!existsSync(path)) {
            return error('RUNTIME_START_FAILED', `This conversation ran in ${path}, and that folder is not there any more.`) as CodexMissionStartResponse
          }
          runFolder = path
          // The teammate's own-branch worktree is cut from THAT folder's repository.
          if (peer !== undefined && options.peerIn !== undefined) peer = (await options.peerIn(peer, runFolder)) ?? peer
        }
      }
      const workspaceId = workspaceIdFor(runFolder)
      // M11: Own branch on and the branch could not be made. Refused HERE,
      // on the path every start shares -- a direct message, a room post, a
      // relayed share, a routine step, a resume -- because the context falls
      // back to the shared folder, and every caller but the direct start used
      // to run there in the teammate's write mode.
      if (peer?.worktreeRefused !== undefined) {
        return error('RUNTIME_START_FAILED', peer.worktreeRefused) as CodexMissionStartResponse
      }
      // The teammate's monthly limit, on the same shared path, before
      // anything is recorded. Its own code: a refusal that proves nothing
      // was sent, which a routine must not hold as "dispatch not confirmed".
      const overLimit = await spendRefusalFor(peer?.self.teammateId)
      if (overLimit !== undefined) return error('SPEND_LIMIT_REACHED', overLimit) as CodexMissionStartResponse
      // Read-only unless the renderer explicitly asked for edits. The host
      // decides the sandbox from this one value; the renderer never passes a
      // sandbox string of its own.
      //
      // Auto is checked against the workspace switch here, not taken on the
      // caller's word. A window left open across a change of mind, or a
      // routine recorded while Auto was on, would otherwise carry the widest
      // sandbox into a workspace that has since said no. Refused rather than
      // quietly narrowed: a person who chose Auto and silently got the folder
      // back would only find out from a run that could not do its job.
      if (mode === 'auto' && !(await (options.autoModeAllowed ?? (async () => false))())) {
        return error(
          'RUNTIME_START_FAILED',
          AUTO_OFF_REFUSAL
        ) as CodexMissionStartResponse
      }
      // Approve-each is workspace-write: approvals only mean something when
      // the run could otherwise act, and the server stops it before each act.
      // It used to fall through to read-only here because it never reached
      // this loop at all -- it had a service of its own -- and three other
      // start paths refused it rather than run it wrong. It runs here now.
      const sandbox: MissionSandbox =
        mode === 'auto'
          ? 'full-access'
          : mode === 'accept-edits' || mode === 'approve-each'
            ? 'workspace-write'
            : 'read-only'
      let resolveStartOperation!: () => void
      const startOperation = new Promise<void>((resolve) => {
        resolveStartOperation = resolve
      })
      startOperations.add(startOperation)
      const owner = ownerKeyOf(peer, slot?.key)
      /*
       * WHAT IS CLAIMED WHILE THIS STARTS (0.551): the conversation, not the
       * teammate. A new conversation claims a key of its own, so two can start
       * side by side; a reply claims the turn it follows, so one conversation
       * still takes its turns one at a time.
       */
      const follows = followUpOf ?? continuation?.missionId
      const claimKey = slot !== undefined ? `slot:${owner}` : follows === undefined ? `new:${owner}:${String((freshClaims += 1))}` : `follows:${follows}`
      let claimed = false
      let referenceRun: string | undefined
      let pendingRunSkills: PreparedRunSkills | undefined
      try {
        if (disposed) {
          return error(
            'RUNTIME_START_FAILED',
            'The mission service is shutting down.'
          ) as CodexMissionStartResponse
        }
        if (!validPrompt(prompt)) {
          return error(
            'INVALID_PROMPT',
            `Enter a mission between 1 and ${MAX_PROMPT_LENGTH.toLocaleString('en-US')} characters.`
          ) as CodexMissionStartResponse
        }
        // ONE TURN AT A TIME PER CONVERSATION (0.551), not one conversation
        // per teammate. It was one live mission per teammate -- "two runs
        // sharing a name would share a workroom voice" -- and a teammate busy
        // in one conversation could not start another. Colin, 2026-10-02:
        // "we should 100% be able to have multiple convos with the same
        // teammate going, that should be a basic day1 feature". Claude Code
        // runs sessions side by side. So only a reply to a turn still running
        // waits (the window queues it); the pool cap still bounds the total.
        //
        // EXCEPT the next turn of a run that has already ended. A run's
        // terminal events reach the window before this service lets go of it
        // -- the disk observation and the share come after -- and a message
        // queued behind that run goes the moment the window sees it end. It
        // was refused here as "already has a mission running", and the queue
        // had nothing left to wait behind, so the message was dropped and the
        // window was left on a run that no longer existed (drive-long-
        // conversation on 0.297, 2026-09-23: turn 5 never ran). A follow-up of
        // that very mission, on the same runtime, is the conversation going
        // on, not a second run beside it. On another runtime it checkpoints
        // the record the bookkeeping is still writing to, so that one waits.
        const winding = (mission: ActiveCodexMission): boolean =>
          mission.settled && followUpOf !== undefined && mission.missionId === followUpOf && mission.runtime === runtime
        const live = [...active.values()].filter((mission) => !winding(mission))
        /*
         * A FIX GOES ONLY FROM THE CONVERSATION'S NEWEST TURN (0.813, Sol's review of 0.812). Locust decides to send a
         * failing check back, then starts it; the person's own reply to the same turn could start in between, and the
         * fix would continue that turn beside it -- two turns at once in one conversation and one folder. Checked here,
         * in the same step that claims the start, against every turn starting or running that continues it.
         */
        if (startedBy?.kind === 'check-fix' && follows !== undefined
          && ([...contextStarting.values()].includes(follows) || live.some((mission) => mission.continues === follows))) {
          return error('RUN_ALREADY_ACTIVE', 'This conversation went on before the fix could start, so Locust did not send the failing check back.') as CodexMissionStartResponse
        }
        // A comparison's column still runs one at a time.
        const ownerBusy = starting.has(claimKey)
          || (slot !== undefined && live.some((mission) => ownerKeyOf(mission.peer, mission.slot) === owner))
          || (follows !== undefined && live.some((mission) => mission.missionId === follows))
        if (ownerBusy) {
          return error(
            'RUN_ALREADY_ACTIVE',
            peer === undefined
              // M27: said to the person now, so it names the real rule, not "Codex".
              ? 'This conversation is still answering. Your message goes when it finishes.'
              : `${peer.self.name} is still answering in this conversation. Your message goes when that turn finishes.`
          ) as CodexMissionStartResponse
        }
        if (starting.size + live.length + (options.liveElsewhere ?? (() => 0))() >= MAX_LIVE_MISSIONS) {
          return error(
            'RUN_ALREADY_ACTIVE',
            `Up to ${MAX_LIVE_MISSIONS} missions can run at once. Wait for one to finish or stop it first.`,
            'pool'
          ) as CodexMissionStartResponse
        }

        const startLifecycleVersion = lifecycleVersion
        starting.add(claimKey)
        claimed = true
        if (continues !== undefined) contextStarting.set(claimKey, continues)
        if (runtime === 'claude') await options.claudeContext?.beforeStart()
        let runtimes: readonly RuntimeDiscovery[] = []
        say('looking')
        /** An agent the person added (0.777): found by its own command line, never by discovery. */
        let acpAgentRun: { readonly agent: AcpAgent; readonly found: AcpLaunch; readonly model?: string } | undefined
        if (runtime === 'acp') {
          const found = await acpAgentFor(mode, chosenModel)
          if ('refusal' in found) return error('RUNTIME_START_FAILED', found.refusal) as CodexMissionStartResponse
          acpAgentRun = found
        } else {
          try {
            runtimes = await options.discover(runtime, ...(route.account === undefined ? [] : [route.account]))
          } catch {
            return error(
              'CODEX_UNAVAILABLE',
              'Codex readiness could not be verified. Check the local runtime and try again.'
            ) as CodexMissionStartResponse
          }
        }

        if (startLifecycleVersion !== lifecycleVersion) {
          return error(
            'RUNTIME_START_FAILED',
            'The Codex mission was stopped before launch.'
          ) as CodexMissionStartResponse
        }

        const chosen: Pick<RuntimeDiscovery, 'availability' | 'readiness' | 'executable'> & { readonly version?: { readonly version: string } } | undefined = acpAgentRun !== undefined
          ? {
              availability: 'available',
              readiness: 'ready',
              executable: acpAgentRun.found.launch,
              ...(acpAgentRun.agent.answer?.version === undefined ? {} : { version: { version: acpAgentRun.agent.answer.version } })
            }
          : runtimes.find((entry) => entry.id === runtime)
        if (
          chosen?.availability !== 'available'
          || chosen.readiness !== 'ready'
          || chosen.executable === undefined
        ) {
          return error(
            'CODEX_UNAVAILABLE',
            notReadyRefusal(runtime, runtimes.find((entry) => entry.id === runtime))
          ) as CodexMissionStartResponse
        }
        const opencodeAcp = runtime === 'opencode' && cloud === undefined && openCodeUsesAcp(chosen.version)
        if (runtime === 'opencode' && cloud !== undefined && openCodeUsesAcp(chosen.version)) {
          return error('RUNTIME_START_FAILED', 'OpenCode 2 runs on this computer through ACP. Turn Cloud off for this conversation, or choose an OpenCode 1 installation for the cloud route.') as CodexMissionStartResponse
        }
        if (opencodeAcp && options.acpSpawn === undefined) {
          return error('RUNTIME_START_FAILED', 'OpenCode 2 needs its ACP connection. Your message has not been sent; reopen Locust and try again.') as CodexMissionStartResponse
        }
        if (runtime === 'cursor') {
          const picked = cursorChoiceFor(cursorModelsFrom(runtimes), chosenModel, route.effort)
          const managedVariant = policyTurnRefusal(options.organizationPolicy, { runtime, mode, model: picked.model })
          if (managedVariant !== undefined) return error('POLICY_REFUSED', managedVariant) as CodexMissionStartResponse
        }
        if (!hostReadsEventsOf(runtime)) {
          return error(
            'RUNTIME_START_FAILED',
            unreadableRuntimeRefusal(runtime)
          ) as CodexMissionStartResponse
        }
        // Only the CLI is run from here: the app's own server takes none of these flags (0.540).
        if (runtime === 'antigravity' && chosen.executable.commandName !== 'agy') {
          return error(
            'RUNTIME_START_FAILED',
            'Antigravity runs here through Antigravity CLI, which is not installed. Settings > AI agents shows how to add it.'
          ) as CodexMissionStartResponse
        }

        /*
         * YOUR CLOUD COMPUTER (0.761): OpenCode on its own server there; Claude Code and Codex too (0.768), each signed
         * in there by the person, Codex on its app server. Refused in words, before anything.
         */
        if (cloud !== undefined) {
          if (runtime !== 'opencode' && runtime !== 'claude' && runtime !== 'codex') {
            return error('RUNTIME_START_FAILED', 'Your cloud computer runs OpenCode, Claude Code and Codex teammates for now. Pick one of their models, or turn Cloud off.') as CodexMissionStartResponse
          }
          if (options.cloud === undefined || (runtime === 'opencode' && options.opencodeServeSpawn === undefined) || (runtime === 'codex' && options.appServerSpawn === undefined)) {
            return error('RUNTIME_START_FAILED', 'Cloud is not available in this window.') as CodexMissionStartResponse
          }
          if (side !== undefined) {
            return error('RUNTIME_START_FAILED', 'A question on the side is asked on this computer, not on your cloud computer.') as CodexMissionStartResponse
          }
          if (chosenModel !== undefined && chosenModel.startsWith('own-')) {
            return error('RUNTIME_START_FAILED', 'A model of your own answers from this computer, so it cannot go to your cloud computer. Pick an OpenCode model, or turn Cloud off.') as CodexMissionStartResponse
          }
        }

        // A reply resumes the earlier mission's own session. The handle comes
        // from the durable record of THAT mission, never from the renderer:
        // the renderer names a mission, and the host decides what that means.
        let resumeThreadId: string | undefined
        /** A side question's running turn, when its copy predates it (0.515). */
        let sideRunningAsk: string | undefined
        let resumedMissionId: string | undefined
        /** The run of the turn a follow-up continues, to tell it when the person undid that turn (0.784). */
        let resumedRunId: string | undefined
        /*
         * A SIDE QUESTION forks the conversation's latest session, read-only,
         * on the conversation's own runtime -- the three that can fork one:
         * Claude Code (--fork-session), Codex (thread/fork, ephemeral) and
         * OpenCode (--fork). It is not the conversation's next turn: no
         * continuation is recorded, and the brief that session already holds
         * is not sent again.
         */
        if (side !== undefined) {
          // Codex copies a thread only through its app-server; its `exec resume`
          // would carry the question into the conversation itself.
          if ((runtime !== 'claude' && runtime !== 'codex' && runtime !== 'opencode') || (runtime === 'codex' && options.appServerSpawn === undefined)) {
            return error('RUNTIME_START_FAILED', `${runtimeDisplayName(runtime)} cannot answer on the side of a conversation: it has no way to copy one.`) as CodexMissionStartResponse
          }
          if (mode !== 'ask') {
            return error('RUNTIME_START_FAILED', 'A question on the side is only ever asked read-only.') as CodexMissionStartResponse
          }
          const forked = await options.ledger.getMission(side.of).catch(() => undefined)
          if (forked === undefined || forked.metadata.runtime !== runtime) {
            return error('RUNTIME_START_FAILED', 'That conversation cannot be asked about on the side.') as CodexMissionStartResponse
          }
          /*
           * The session to copy: the turn's own, or -- for a turn still
           * running, which has not said its session yet -- the one it
           * resumed, back through the conversation (drive-side-chat, 0.461:
           * asked mid-turn, the running turn named none).
           */
          let forkedThread: string | undefined
          let at: typeof forked | undefined = forked
          for (let hops = 0; at !== undefined && forkedThread === undefined && hops < 8; hops += 1) {
            forkedThread = runtimeThreadIdOf(at) ?? at.metadata.continuesFrom?.runtimeThreadId
            const earlier: string | undefined = at.metadata.continuesFrom?.missionId
            at = forkedThread !== undefined || earlier === undefined ? undefined : await options.ledger.getMission(earlier).catch(() => undefined)
            if (at !== undefined && at.metadata.runtime !== runtime) at = undefined
          }
          if (forkedThread === undefined) {
            return error('RUNTIME_START_FAILED', 'This conversation has no session to ask about yet. Ask again once its reply has started.') as CodexMissionStartResponse
          }
          resumeThreadId = forkedThread
          /*
           * THE TURN STILL RUNNING IS SAID (0.515). Copied from the session
           * it resumed, the side copy holds the conversation up to the turn
           * BEFORE the running one -- a pass on 0.512 asked on the side
           * during a rename and was told "this conversation itself never
           * discussed a rename". When the turn has no session of its own yet,
           * what it was asked goes with the question.
           */
          if (runtimeThreadIdOf(forked) === undefined) sideRunningAsk = forked.metadata.prompt
        }
        // Whether the session being resumed compacted during that turn: then
        // it holds a summary, not the brief, and this turn is briefed in full.
        let resumedCompacted = false
        /** A reply this runtime cannot resume: the conversation so far goes with it (0.495). */
        let coldEarlier: readonly EarlierTurn[] | undefined
        if ((opencodeAcp || side?.freshSession === true) && side !== undefined) {
          // ACP's portable turn path does not fork, and a kept comparison column whose temporary worktree or copy was
          // removed on Keep recorded its session in a folder that is no longer there. Brief a fresh session so the side
          // turn can never append to a live parent or fail on the gone copy directory.
          const prior = await options.ledger.getMission(side.of)
          if (prior !== undefined) {
            const said = createTranscriptTracker()
            said.track(prior.events)
            coldEarlier = [...(await earlierTurnsOf(prior)), { asked: prior.metadata.prompt, answered: said.latestFinal }]
          }
          resumeThreadId = undefined
        }
        // What the person did in the runtime's own terminal on this
        // conversation lands first (0.391), so this turn continues from the
        // last of it. After the busy check above, which knows the settled
        // turn by the id the window sent.
        if (followUpOf !== undefined && options.catchUpTerminal !== undefined && rewind !== true) {
          const named = followUpOf
          followUpOf = (await options.catchUpTerminal(named).catch(() => undefined))?.latestMissionId ?? named
        }
        if (followUpOf !== undefined) {
          const prior = await options.ledger.getMission(followUpOf).catch(() => undefined)
          if (prior === undefined) {
            return error(
              'RUNTIME_START_FAILED',
              'That conversation cannot be continued: its earlier mission is not in the ledger.'
            ) as CodexMissionStartResponse
          }
          if (rewind === true) {
            /*
             * STARTED AGAIN FROM AN EDITED MESSAGE (0.498, Claude Code's
             * Esc Esc rewind). `followUpOf` is the turn BEFORE the one the
             * person edited, and its session also holds every turn after it,
             * so resuming it would hand the model the very turns being set
             * aside. Cold, then, on whichever runtime is chosen, with the
             * conversation up to that turn as a runtime switch carries it.
             */
            resumedMissionId = prior.metadata.missionId
            const said = createTranscriptTracker()
            said.track(prior.events)
            coldEarlier = [...(await earlierTurnsOf(prior)), { asked: prior.metadata.prompt, answered: said.latestFinal }]
          } else if (prior.metadata.runtime !== runtime || accountFromRoute(prior.metadata.resolvedRouteId) !== route.account) {
            // A reply on ANOTHER runtime. This used to be refused ("switch the
            // route back, or hand the mission over"), and the person's
            // workaround was a fresh mission with the filenames and the task
            // retyped -- the 0.21.2 QA pass did exactly that after a quota
            // failure. It is a handoff without the stop: the earlier mission
            // is reconciled, the briefing carries what was done and what was
            // left unsettled, and the reply rides on it as the latest word.
            let checkpoint
            try {
              checkpoint = await options.ledger.createCheckpoint(prior.metadata.missionId, 'route-switch')
            } catch {
              return error(
                'RUNTIME_START_FAILED',
                'That conversation could not be reconciled, so it cannot be continued on another runtime.'
              ) as CodexMissionStartResponse
            }
            if (checkpoint.resumeSafety === 'unsafe') {
              return error(
                'RUNTIME_START_FAILED',
                `That conversation cannot be continued safely on another runtime: ${checkpoint.safetyReason}`
              ) as CodexMissionStartResponse
            }
            // A long task by file, not clipped (0.513, long-task-file.ts).
            const taskFile = await longTaskFile(prior.metadata.prompt, await projectFolderOf(options, prior.metadata.missionId), prior.metadata.missionId)
            const composed = composeHandoffPrompt(
              prior.metadata.prompt,
              checkpoint,
              runtimeDisplayName(prior.metadata.runtime),
              prompt,
              await earlierTurnsOf(prior),
              taskFile,
              leaveOut,
              howTurnEnded(prior.events)
            )
            const briefing = composed === undefined || taskFile === undefined ? composed : { ...composed, prompt: withAttachments(composed.prompt, [taskFile]) }
            if (briefing === undefined) {
              return error(
                'RUNTIME_START_FAILED',
                'That conversation is too long to carry to another runtime with this reply.'
              ) as CodexMissionStartResponse
            }
            // Re-enter as a route-switch continuation of the prior mission:
            // the same record the live handoff writes, so the thread draws
            // the same divider and a reopened conversation stitches the same
            // way. The briefing, not the bare reply, starts the new runtime.
            // This call holds the owner's in-flight claim; the re-entry
            // would see it and refuse itself, so it is released first.
            if (claimed) {
              starting.delete(claimKey)
              claimed = false
            }
            const switched = await service.start(
              briefing.prompt,
              runtime,
              mode,
              route,
              emit,
              { missionId: prior.metadata.missionId, checkpointEpoch: checkpoint.epoch, reason: 'route-switch', ...(briefing.omitted.length === 0 ? {} : { leftOut: briefing.omitted }), ...(briefing.leftOutByYou.length === 0 ? {} : { leftOutByYou: briefing.leftOutByYou }) },
              peer,
              undefined,
              relay,
              startedBy,
              undefined,
              undefined,
              undefined,
              undefined,
              undefined,
              cloud
            )
            if (!switched.ok) return switched
            // The receipt says it was a switch, so the window can draw the
            // seam the moment it happens. It used to come back looking like
            // any other reply, and the divider appeared only once the
            // conversation was rebuilt from the record.
            return {
              ok: true,
              data: {
                ...switched.data,
                switchedFrom: {
                  missionId: prior.metadata.missionId,
                  runtime: prior.metadata.runtime,
                  model: prior.metadata.model,
                  unsettledCount: checkpoint.unsettledActions.length,
                  omittedBriefing: briefing.omitted,
                  ...(briefing.leftOutByYou.length === 0 ? {} : { leftOutByYou: briefing.leftOutByYou })
                }
              }
            }
          }
          // A prior turn that failed before its runtime started recorded no
          // session. That used to refuse the reply outright; now the turn is
          // recorded as continuing that conversation and the runtime simply
          // starts fresh, because a person replying to a failure is still
          // replying to it. `resumeThreadId` staying undefined is what makes
          // the run cold: no `exec resume`, no borrowed context.
          // A resumed session keeps the tools it was BUILT with, so a mode
          // the person changed since cannot reach it: the chip said "Accept
          // edits" while the runtime still answered "this session has no
          // write, edit, or bash tools" (outside tester, 2026-09-07).
          //
          // So a changed mode starts cold rather than resuming. The thread
          // already says what that costs -- "Started without the earlier
          // messages" -- and that is a smaller thing to lose than the
          // guarantee that the mode on screen is the mode being run.
          if (rewind !== true) {
          const priorMode = prior.metadata.mode
          const modeChanged = priorMode !== undefined && priorMode !== mode
          const protocolChanged = runtime === 'opencode' && openCodeUsesAcp(typeof prior.metadata.cliVersion === 'string' ? { version: prior.metadata.cliVersion } : undefined) !== openCodeUsesAcp(chosen.version)
          // Imported sessions may hold permissions granted outside Locust.
          // Only this host's ACP turns are known to have used allow-once.
          const unverifiedAcpSession = opencodeAcp && (priorMode === undefined || prior.metadata.command?.args.at(-1) !== 'acp')
          /*
           * A TURN STOPPED BEFORE IT NAMED ITS SESSION (0.496) was itself a
           * resume of the turn before it, so that session is still the
           * conversation: the reply picks it up, as Claude Code does after an
           * interrupt. It used to start cold -- "A fresh session" in the
           * thread -- over a Stop pressed half a second into a reply. The
           * same walk the side question makes; recorded only when the stopped
           * turn really did resume one.
           */
          resumeThreadId = modeChanged || protocolChanged || unverifiedAcpSession ? undefined : resumableThreadOf(prior, runtime)
          resumedMissionId = prior.metadata.missionId
          resumedRunId = prior.metadata.runId
          resumedCompacted = compactedDuring(prior.events)
          if (resumeThreadId === undefined) {
            const said = createTranscriptTracker()
            said.track(prior.events)
            // Send again: the same words, unanswered, are this turn -- not an earlier one to quote back.
            const again = (said.latestFinal ?? '').trim().length === 0 && typeof prior.metadata.prompt === 'string' && prior.metadata.prompt.trim() === prompt.trim()
            coldEarlier = [...(await earlierTurnsOf(prior)), ...(again ? [] : [{ asked: prior.metadata.prompt, answered: said.latestFinal }])]
          }
          }
        }
        /*
         * HERE OR THERE (0.761). This computer and the cloud one each keep their own OpenCode sessions, so a reply
         * that moves between them cannot resume the other's: it starts afresh, told what was said before, as a reply
         * on another runtime is.
         */
        const priorCloud = followUpOf === undefined || options.cloud === undefined ? undefined : await options.cloud.of(followUpOf).catch(() => undefined)
        if (followUpOf !== undefined && side === undefined && rewind !== true && (priorCloud !== undefined) !== (cloud !== undefined) && resumeThreadId !== undefined) {
          const prior = await options.ledger.getMission(followUpOf).catch(() => undefined)
          if (prior !== undefined) {
            const said = createTranscriptTracker()
            said.track(prior.events)
            coldEarlier = [...(await earlierTurnsOf(prior)), { asked: prior.metadata.prompt, answered: said.latestFinal }]
          }
          resumeThreadId = undefined
        }

        const runId = `run_${createId()}`
        const missionId = `mission_${createId()}`
        const controller = new AbortController()
        const flushBeforeApproval = async (): Promise<void> => {
          await active.get(runId)?.flushEvents?.()
          if (controller.signal.aborted) throw new Error('The run stopped before the approval could be shown.')
        }
        const createdAt = now().toISOString()
        const routeId =
          runtime === 'claude'
            ? 'claude'
            : runtime === 'opencode'
              ? 'opencode'
              : runtime === 'copilot'
                ? 'copilot'
                : runtime === 'muse'
                  ? 'muse'
                  : runtime === 'antigravity'
                    ? 'antigravity'
                    : 'codex'
        // ONE definition of what this run may touch, computed before anything
        // records it, so the durable header and the receipt agree with what
        // the process was actually allowed to do.
        //
        // Claude Code used to be forced to `read-only` here on the reading
        // that it takes no sandbox. It takes a different one: the permission
        // mode and the tool list, which `createClaudePrintCommand` now sets
        // from this very value -- `acceptEdits` with the editing tools named,
        // or `plan` with a reading list. MEASURED 2026-09-03: while this line
        // still forced read-only, a mission started in Accept edits recorded
        // `sandbox: read-only`, ran in plan mode, and its Write came back
        // "No such tool available: Write. Write is disabled for this
        // session." The composer offered a mode the run never received.
        const effectiveSandbox: MissionSandbox = sandbox
        // Cursor's plan mode asks a model not to write; only its sandbox stops
        // one, and that sandbox exists on macOS and Linux alone. Measured on
        // Windows: a plan-mode run told firmly to write created two files.
        // Rather than record `read-only` over a run that can edit, the mission
        // is refused and the person is told which mode does what they meant.
        if (
          runtime === 'cursor'
          && effectiveSandbox === 'read-only'
          && !cursorCanEnforceReadOnly(hostPlatform)
        ) {
          return error('RUNTIME_START_FAILED', CURSOR_READ_ONLY_REFUSAL) as CodexMissionStartResponse
        }
        /*
         * A MODEL OF THE PERSON'S OWN (0.357) is declared to OpenCode for this
         * run only, key and all, in the child's environment. One that has
         * since been removed is refused in words before anything is written:
         * OpenCode itself would fail with "model not found" in a log nobody
         * reads.
         */
        const own =
          runtime === 'opencode' && chosenModel !== undefined && chosenModel.startsWith('own-')
            ? await options.ownProvider?.(chosenModel).catch(() => undefined)
            : undefined
        if (runtime === 'opencode' && chosenModel !== undefined && chosenModel.startsWith('own-') && own === undefined) {
          return error(
            'RUNTIME_START_FAILED',
            'That model is no longer in Your own models (Settings, Runtimes & accounts). Pick another, or add it again.'
          ) as CodexMissionStartResponse
        }
        const providers = own === undefined ? undefined : { [own.id]: own.provider }
        // A model of your own its provider lists is priced step by step from that list (0.810, listed-prices.ts).
        const ownPrice = own === undefined ? undefined : own.provider.prices?.[own.provider.models[0] ?? '']
        const resolvedRouteId = `${routeId}-account:${route.account ?? 'default'}`
        const normalizerContext = {
          runId,
          missionId,
          requestedRouteId: routeId,
          resolvedRouteId,
          ...(route.account === undefined ? {} : { account: route.account }),
          ...(chosen.version?.version === undefined ? {} : { cliVersion: chosen.version.version }),
          // A free model's refusal is OpenCode's side, never a sign-in to renew (opencode-error.ts, 2026-10-12).
          ...(runtime === 'opencode' && chosenModel !== undefined ? { model: chosenModel } : {}),
          ...(ownPrice === undefined ? {} : { price: ownPrice }),
          now,
          // What Antigravity's mode refuses, so only a call it could have refused waits for the result (0.551).
          ...(runtime === 'antigravity' ? { sandbox: effectiveSandbox } : {}),
          // Claude Code lists its commands at the start of a run (0.426).
          ...(runtime === 'claude' && options.onRuntimeCommands !== undefined
            ? { onCommands: (commands: readonly RuntimeCommandInfo[]) => options.onRuntimeCommands?.('claude', commands) }
            : {})
        }
        // Copilot resumes by a session id the HOST chooses on a first run, so
        // the host mints it here and hands the same id to the normalizer; the
        // receipt then names a session that can really be resumed even if the
        // stream never echoes it back.
        const copilotSessionId = runtime === 'copilot' ? (resumeThreadId ?? randomUUID()) : undefined
        // Codex speaks app-server whenever the host can start one. The two
        // normalizers read different protocols, so this decision and the
        // command built below are the same decision and are made from the
        // same value.
        const codexStreams = runtime === 'codex' && options.appServerSpawn !== undefined
        /*
         * OpenCode through its own server, in every mode (0.677). It asks only through the server -- Approve each --
         * and its replies stream only through it: through `run` a reply arrived once finished, and on the free
         * models the person read nothing for 20 to 60 seconds, then all of it (the 2026-10-06 sweep). Every other
         * mode is told on the server exactly what `run` told it (openCodeModeEnv), and answers what OpenCode asks
         * as `run` did: Auto approves what the config does not deny, the rest refuse. A side question forks its
         * session, which only `run` does here, so it stays there.
         */
        const opencodeServes = runtime === 'opencode' && !opencodeAcp && options.opencodeServeSpawn !== undefined && (mode === 'approve-each' || side === undefined)
        // Copilot asks only over the Agent Client Protocol (0.377); every
        // other Copilot mode stays on `-p`, which is what they were measured on.
        const copilotAcp = runtime === 'copilot' && mode === 'approve-each' && options.acpSpawn !== undefined
        // An agent the person added speaks only the protocol (0.777); acpAgentFor refused it without acpSpawn.
        const ownAcp = acpAgentRun !== undefined && options.acpSpawn !== undefined
        // What the run has announced it will change, by item id. Read off the
        // stream so a file-change tool row can carry its diff, and so an
        // approval card for that item can show what it is about.
        const changesByItem = new Map<string, readonly FileChangeRecord[]>()
        const appServerNormalizer = (): CodexEventNormalizer => {
          const inner = asProcessNormalizer(createAppServerEventNormalizer({ ...normalizerContext, runtime: 'codex' }))
          return {
            get runtimeThreadId() {
              return inner.runtimeThreadId
            },
            get finalized() {
              return inner.finalized
            },
            accept: (record) => {
              const notification = notificationOfRecord(record)
              const found = notification === undefined ? undefined : itemOf(notification.params)
              if (found !== undefined) {
                const changes = fileChangesOf(found.item)
                if (changes !== undefined) changesByItem.set(found.id, changes)
              }
              // A call the person declined reads as declined, however Codex put it (0.374).
              return withDeclines(withFileChanges(inner.accept(record), changesByItem, runCwd), options.approvals?.declined(runId))
            },
            finish: (completion) => inner.finish(completion)
          }
        }
        const normalizer = codexStreams
          ? appServerNormalizer()
          : runtime === 'claude'
          ? createClaudeEventNormalizer(normalizerContext)
          : runtime === 'cursor'
            ? createCursorEventNormalizer(normalizerContext)
            : opencodeAcp
              ? createAcpEventNormalizer({ ...normalizerContext, runtime: 'opencode', ...(resumeThreadId === undefined ? {} : { sessionId: resumeThreadId }) })
            : runtime === 'opencode'
              ? createOpenCodeEventNormalizer(normalizerContext)
              : copilotAcp || ownAcp
                // The session is the agent's to name on a first run; the
                // stream's first record says it (acp-events.ts).
                ? createAcpEventNormalizer({ ...normalizerContext, runtime: ownAcp ? 'acp' : 'copilot', ...(resumeThreadId === undefined ? {} : { sessionId: resumeThreadId }) })
              : runtime === 'copilot'
                ? createCopilotEventNormalizer({ ...normalizerContext, sessionId: copilotSessionId! })
                : runtime === 'muse'
                  ? createMuseEventNormalizer(normalizerContext)
                  : runtime === 'antigravity'
                    ? createAgyEventNormalizer(normalizerContext)
                    : createCodexEventNormalizer(normalizerContext)

        // The argv, decided BEFORE anything durable is written. The builders
        // refuse what they cannot honour -- an effort for a runtime that has
        // no effort flag, a model that is not plain text -- and a refusal
        // there used to leave a mission file and a failure record behind for
        // a run that never existed. Nothing created today can be pruned
        // today, so those files were permanent.
        // Narrowed above; captured so the closure below keeps the narrowing.
        const executable = chosen.executable
        // The teammate's own worktree when it has one, else the folder. The
        // ledger's workspace id stays the FOLDER's: history is per folder.
        // A comparison's column may answer in a copy of the folder (0.443, compare-copies.ts).
        const runCwd = slot?.cwd ?? peer?.cwd ?? runFolder
        const references = await options.references?.select(continues, referenceIds) ?? []
        const referenceFolders = references.filter((item) => referenceAccess(runtime, mode, item.path, runCwd, cloud !== undefined).include).map((item) => item.path)
        if (references.length > 0) {
          options.references?.hold(runId, references.map((item) => item.id))
          referenceRun = runId
        }
        // Named by whoever made the worktree, not inferred from this folder:
        // a teammate with its own folder is in a worktree of a repository
        // that is not this one. Absent unless the run really is in a
        // worktree.
        const repositoryRoot = cloud === undefined ? peer?.repositoryRoot : undefined
        /*
         * Somewhere for a Claude Code run to ASK before using a connector.
         *
         * Auto asks nothing, so it gets no bridge. Every other mode used to
         * have nowhere to put the question and refused every connector call;
         * then 0.61.0 allowed them all without asking. This is the asking:
         * the run is registered with the host, which mints it a token and an
         * mcp.json, and the card the app already has answers.
         */
        const askEveryConnector =
          globalThis.process.env.LOCUST_ASK_CONNECTORS === '1' || (await options.askConnectors?.()) === true
        // A cloud turn's guard is the computer it runs on (cloud-computer.ts): the guards below name files on this one.
        const commandGuardPath = runtime === 'claude' && cloud === undefined ? await options.commandGuard?.() : undefined
        /*
         * A SEALED RUN READS ONLY WHAT ITS PROMPT GIVES IT (0.824, Scout's look; Grok's review, #4). Its prompt holds
         * other people's words -- pull request titles, check output, comments -- so the run gets no project-memory
         * brief, no browser, no skills and no git tool; OpenCode refuses it every tool and Codex starts it with
         * none (codexSealedThreadConfig). Elsewhere the runtime's own Ask holds it.
         */
        const sealed = slot?.sealed === true
        const copilotGuardPlugin = runtime === 'copilot' ? await options.copilotCommandGuard?.().catch(() => undefined) : undefined
        const copilotMcpServersOff = runtime === 'copilot' && effectiveSandbox === 'read-only'
          ? await options.copilotMcpServers?.().catch(() => undefined)
          : undefined
        // A cloud turn's guard is the computer it runs on: nothing there reaches GitHub (cloud-computer.ts).
        const openCodeGuardPlugin = runtime === 'opencode' && cloud === undefined
          ? await (opencodeAcp ? options.openCode2CommandGuard?.() : options.openCodeCommandGuard?.())?.catch(() => undefined)
          : undefined
        if (opencodeAcp && openCodeGuardPlugin === undefined) {
          return error('RUNTIME_START_FAILED', 'OpenCode 2 could not prepare the command guard. Your message has not been sent; reopen Locust and try again.') as CodexMissionStartResponse
        }
        // Ask and Plan on OpenCode have no shell, so git reaches them through Locust's git tool (0.818).
        const openCodeGitReads = runtime === 'opencode' && !opencodeAcp && cloud === undefined && effectiveSandbox === 'read-only' && !sealed
          ? await options.openCodeGitReads?.().catch(() => undefined)
          : undefined
        const codexGuardConfig = runtime === 'codex' && cloud === undefined ? await options.codexCommandGuard?.(runCwd, chosen.executable.env?.CODEX_HOME).catch(() => undefined) : undefined
        // The person's connectors are this computer's: a turn there has none to ask about.
        const permissionBridge =
          cloud === undefined && runtime === 'claude' && effectiveSandbox !== 'full-access' && options.permissionHost !== undefined
            ? await options.permissionHost.register({ runId, missionId, cwd: runCwd, beforeApproval: flushBeforeApproval })
            : undefined
        const browserRuntime = supportsPerRunBrowserMcp(runtime) ? runtime : undefined
        const browserApprovalHandler =
          cloud === undefined && browserRuntime !== undefined && options.approvals !== undefined
            ? options.approvals.requestHandlerFor({ runId, missionId, cwd: runCwd, changesByItem, runtime: browserRuntime })
            : undefined
        const browserMcp =
          cloud === undefined &&
          !sealed &&
          browserRuntime !== undefined &&
          options.browserHost !== undefined &&
          policyFeatureRefusal(options.organizationPolicy, 'browser') === undefined
            ? await options.browserHost.registerRun({
                runId,
                missionId,
                teammateLabel: peer?.self.name ?? runtimeDisplayName(runtime),
                cwd: runCwd,
                ...(browserApprovalHandler === undefined
                  ? {}
                  : {
                      askApproval: async (decision) => {
                        await flushBeforeApproval()
                        const res = await browserApprovalHandler({
                          id: 0,
                          method: 'item/commandExecution/requestApproval',
                          params: {
                            command: decision.detail,
                            cwd: runCwd,
                            locustAlwaysKey: `browser:${decision.gate}:${decision.summary}`,
                            locustCard: {
                              summary: decision.summary,
                              alwaysCovers: `this same browser action (${decision.gate}) during this session`,
                              dataSentSays: 'The browser action above runs in Locust’s isolated Browser panel.',
                              reversibleSays: 'Submitting a form, sending, paying, or downloading cannot be undone by Locust.'
                            }
                          }
                        })
                        const rec = typeof res === 'object' && res !== null && !Array.isArray(res) ? (res as Record<string, unknown>) : {}
                        const dec = rec.decision
                        return {
                          approved: dec === 'accept' || dec === 'acceptForSession',
                          always: dec === 'acceptForSession',
                          ...(typeof rec.reason === 'string' && rec.reason.length > 0 ? { reason: rec.reason } : {})
                        }
                      }
                    })
              }).catch(() => undefined)
            : undefined
        /*
         * SKILLS FOR CODEX, OPENCODE, COPILOT AND ANTIGRAVITY (0.813, run-skills.ts): Codex on its app-server, which takes
         * extra skill folders for its process; OpenCode in its per-run config's skills list; Copilot as one plugin
         * directory; Antigravity as one absolute --add-dir. Not on a cloud computer, where the copy would be on the
         * wrong machine.
         */
        const runSkills =
          cloud === undefined && options.runSkills !== undefined && !sealed && ((runtime === 'codex' && codexStreams) || runtime === 'opencode' || runtime === 'copilot' || runtime === 'antigravity')
            ? await options.runSkills({
              ...(peer?.self.teammateId === undefined ? {} : { teammateId: peer.self.teammateId }),
              ...(slot?.skill === undefined ? {} : { skill: slot.skill })
            }).catch(() => undefined)
            : undefined
        pendingRunSkills = runSkills
        let skillPaths: readonly string[] | undefined
        if (runtime === 'opencode' && !opencodeAcp && runSkills !== undefined) {
          try {
            skillPaths = await openCodeSkillPaths({ cwd: runCwd, ...(executable.env === undefined ? {} : { env: { ...globalThis.process.env, ...executable.env } }) })
          } catch (cause) {
            // Its own refusals name the file to fix.
            return error('RUNTIME_START_FAILED', cause instanceof Error ? cause.message : 'OpenCode’s settings couldn’t be read, so nothing was started.') as CodexMissionStartResponse
          }
        }
        if (runSkills !== undefined) options.note?.('skills-given', `${missionId} ${runSkills.skills.join(' ')}`)
        const openCodeAcpLaunch = !opencodeAcp ? undefined : openCode2Launch(executable, {
          workspacePath: runCwd,
          sandbox: effectiveSandbox,
          approveEach: mode === 'approve-each',
          referenceFolders,
          ...(repositoryRoot === undefined ? {} : { repositoryRoot }),
          ...(providers === undefined ? {} : { providers }),
          ...(openCodeGuardPlugin === undefined ? {} : { guardPlugin: openCodeGuardPlugin }),
          ...(runSkills === undefined ? {} : { skillsDir: runSkills.skillsDir })
        })
        /*
         * SKILLS (0.679). `--restricted` keeps a Claude run from finding the
         * folder's skills or the person's, so they are handed to it as
         * plugins, copied for this run. Auto finds them itself -- all but
         * the skills kept from GitHub (0.710), which live in Locust's profile.
         */
        const skills =
          runtime === 'claude' && cloud === undefined && !sealed && options.claudeSkills !== undefined
            ? await options.claudeSkills(runCwd, {
              auto: effectiveSandbox === 'full-access',
              // The teammate's own skills (0.811), and a tune column's one skill for this run alone.
              ...(peer?.self.teammateId === undefined ? {} : { teammateId: peer.self.teammateId }),
              ...(slot?.skill === undefined ? {} : { skill: slot.skill })
            }).catch(() => undefined)
            : undefined
        const skillPlugins = skills?.plugins.map((plugin) => plugin.dir) ?? []
        if (skills !== undefined && skills.plugins.length > 0) {
          options.note?.('skills-given', `${missionId} ${skills.plugins.flatMap((plugin) => plugin.skills).join(' ')}`)
        }
        let command: RuntimeCommandSpec
        // OpenCode and Copilot take the prompt as an argument, not on stdin,
        // so their argv is built once now with the person's own words -- so a
        // builder's refusal still lands before anything durable -- and again
        // below with the prompt the runtime is actually sent.
        // A command the person typed goes to the runtime bare (0.426). OpenCode
        // takes it by name, `--command init` or its server's command call,
        // and only what follows the name as the message (0.427).
        // Codex's go as its own requests, which only its app-server takes (0.428).
        const bare = asCommand === true && relay === undefined && startedBy === undefined && (runtime !== 'codex' || codexStreams)
        const openCodeCommand = bare && runtime === 'opencode' ? commandNamed(prompt) : undefined
        const codexCommand = bare && runtime === 'codex' ? commandNamed(prompt) : undefined
        /** Claude Code or Codex on the cloud computer (0.768): its own copy there, in the conversation's folder there. */
        let cloudLaunch: { readonly executable: ExecutableLaunch; readonly dir: string } | undefined
        const buildCommand = (promptText: string): RuntimeCommandSpec => {
          // Cursor's level lives in its id: turned into it from this run's own list (cursorChoiceFor).
          const picked = runtime === 'cursor' ? cursorChoiceFor(cursorModelsFrom(runtimes), chosenModel, route.effort) : { model: chosenModel, effort: route.effort }
          const chosenEffort = picked.effort
          const agentExecutable = cloudLaunch?.executable ?? executable
          const agentFolder = cloudLaunch?.dir ?? runCwd
          const choice = {
            referenceFolders,
            ...(picked.model === undefined ? {} : { model: picked.model }),
            ...(chosenEffort === undefined ? {} : { effort: chosenEffort }),
            ...(resumeThreadId === undefined ? {} : { resumeThreadId }),
            ...(side === undefined ? {} : { forkSession: true })
          }
          if (opencodeAcp) return createOpenCodeAcpCommand(executable, runCwd, openCodeAcpLaunch)
          if (runtime === 'opencode' && opencodeServes) {
            return createOpenCodeServeCommand(executable, {
              referenceFolders,
              workspacePath: runCwd,
              // Approve each asks for everything; any other mode, what `run` would have been told.
              ...(mode === 'approve-each' ? {} : { sandbox: effectiveSandbox }),
              ...(repositoryRoot === undefined ? {} : { repositoryRoot }),
              ...(providers === undefined ? {} : { providers }),
              ...(openCodeGuardPlugin === undefined ? {} : { openCodeGuardPlugin }),
              ...(browserMcp === undefined ? {} : { browserMcp }),
              ...(openCodeGitReads === undefined ? {} : { gitReads: openCodeGitReads }),
              ...(sealed ? { sealed: true } : {}),
              ...(runSkills === undefined ? {} : { skillsDir: runSkills.skillsDir, skillPaths })
            })
          }
          if (runtime === 'opencode') {
            return createOpenCodeRunCommand(executable, {
              workspacePath: runCwd,
              sandbox: effectiveSandbox,
              prompt: promptText,
              ...(openCodeCommand === undefined ? {} : { slashCommand: openCodeCommand }),
              // Only when this run is in a worktree, which is exactly when
              // the folder it stands in is not the repository it belongs to.
              ...(repositoryRoot === undefined ? {} : { repositoryRoot }),
              ...(providers === undefined ? {} : { providers }),
              ...(openCodeGuardPlugin === undefined ? {} : { openCodeGuardPlugin }),
              ...(browserMcp === undefined ? {} : { browserMcp }),
              ...(openCodeGitReads === undefined ? {} : { gitReads: openCodeGitReads }),
              ...(sealed ? { sealed: true } : {}),
              ...(runSkills === undefined ? {} : { skillsDir: runSkills.skillsDir, skillPaths }),
              ...choice
            })
          }
          if (runtime === 'antigravity') {
            // Antigravity CLI (0.540): the prompt goes on stdin as its own stream-json line.
            return createAgyPrintCommand(executable, {
              referenceFolders,
              workspacePath: runCwd,
              sandbox: effectiveSandbox,
              ...agyRoute(chosenModel, route.effort),
              ...(resumeThreadId === undefined ? {} : { resumeThreadId }),
              ...(runSkills === undefined ? {} : { skillWorkspace: runSkills.dir })
            })
          }
          if (runtime === 'muse') {
            // The prompt is named here only so the builder can refuse an
            // empty one before anything durable is written. What Muse is
            // SENT is the file the runner writes from `runtimePrompt`, which
            // is the same text every other runtime receives.
            return createMuseExecCommand(executable, {
              workspacePath: runCwd,
              sandbox: effectiveSandbox,
              prompt: promptText,
              ...choice
            })
          }
          if (runtime === 'acp' && acpAgentRun !== undefined) {
            // The person's own command line (0.777); the prompt, the session and the model travel over the protocol.
            return createAcpAgentCommand(executable, acpAgentRun.found.args, runCwd)
          }
          if (runtime === 'copilot' && copilotAcp) {
            // The prompt and the session travel over the protocol, not the argv.
            return createCopilotAcpCommand(executable, {
              workspacePath: runCwd,
              ...(chosenModel === undefined || chosenModel === 'auto' ? {} : { model: chosenModel }),
              ...(chosenEffort === undefined ? {} : { effort: chosenEffort }),
              ...(copilotGuardPlugin === undefined ? {} : { guardPlugin: copilotGuardPlugin }),
              ...(runSkills === undefined ? {} : { skillPlugin: runSkills.dir })
            })
          }
          if (runtime === 'copilot') {
            return createCopilotPromptCommand(executable, {
              workspacePath: runCwd,
              sandbox: effectiveSandbox,
              prompt: promptText,
              ...(chosenModel === undefined || chosenModel === 'auto' ? {} : { model: chosenModel }),
              ...(chosenEffort === undefined ? {} : { effort: chosenEffort }),
              ...(resumeThreadId === undefined ? { sessionId: copilotSessionId } : { resumeThreadId }),
              ...(copilotGuardPlugin === undefined ? {} : { guardPlugin: copilotGuardPlugin }),
              ...(copilotMcpServersOff === undefined || copilotMcpServersOff.length === 0 ? {} : { copilotMcpServersOff }),
              ...(browserMcp === undefined ? {} : { browserMcp }),
              ...(runSkills === undefined ? {} : { skillPlugin: runSkills.dir })
            })
          }
          return runtime === 'claude'
            ? createClaudePrintCommand(agentExecutable, {
                workspacePath: agentFolder,
                ...(permissionBridge === undefined ? {} : { permissionBridge }),
                ...(browserMcp === undefined ? {} : { browserMcp }),
                ...(commandGuardPath === undefined ? {} : { commandGuard: { settingsPath: commandGuardPath } }),
                ...(skillPlugins.length === 0 ? {} : { skillPlugins }),
                // Every connector the person's own Claude Code can reach.
                // Without a named allow rule the tools are offered and every
                // call is refused, because a printed run has nowhere to put
                // the approval question.
                ...(() => {
                  /*
                   * `LOCUST_ASK_CONNECTORS=1` sends NO allow rules, so every
                   * connector call goes to the permission host and the person
                   * is asked. Today it is the seam the host is driven through
                   * -- with the rules in place a covered connector never asks,
                   * which is Colin's ruling and the normal state. It is also
                   * the shape of a future "ask me each time" setting.
                   */
                  /*
                   * ASK ASKS (0.545). MEASURED 2026-10-03 with a test server: in
                   * its own plan mode Claude Code asks before every connector
                   * tool, even one marked read-only, and Codex's read-only
                   * sandbox refuses any that change things. Locust's Ask had
                   * pre-approved every ticked connector, so a teammate in Ask
                   * could call one that changes things without a word. Colin,
                   * 2026-10-03: "i just want it to work the way the actual
                   * models do." So Ask sends no rules: each call is a card.
                   */
                  // The teammate's own ticks, and nothing for a teammate with none: its connectors are off (0.798, below).
                  return claudeConnectorRules(effectiveSandbox, askEveryConnector, cloud !== undefined ? [] : (peer?.connectors ?? []))
                })(),
                /*
                 * A TEAMMATE'S CONNECTORS ARE ON ONLY WHEN IT HAS ONE TICKED (0.798). Claude Code sends every tool of
                 * every connector with each call -- about 170k tokens a turn on Colin's account, MEASURED 2026-10-19 on
                 * Haiku, Sonnet and Opus -- so a teammate with none ticked runs without them, ten times lighter. Colin,
                 * asked: "sounds like the logical decision for best user experience right?"
                 */
                accountConnectors: connectorsOn(cloud === undefined ? peer?.connectors : undefined),
                // Claude's containment IS this value: it picks the permission
                // mode and the tool list. Leaving it out defaulted every
                // Claude run to read-only, so a mission started in Accept
                // edits ran in plan mode and answered "I don't have a Write
                // tool available in this session" -- measured 2026-09-03.
                sandbox: effectiveSandbox,
                ...choice
              })
            : runtime === 'cursor'
              ? createCursorPrintCommand(executable, {
                  workspacePath: runCwd,
                  sandbox: effectiveSandbox,
                  platform: hostPlatform,
                  ...choice
                })
              : codexStreams
              ? createCodexAppServerCommand(agentExecutable, {
                  workspacePath: agentFolder,
                  sandbox: effectiveSandbox,
                  // Switches Codex's plan tool on, so its plan reaches the
                  // panel rather than being typed into the reply (0.304).
                  ...(chosen?.version?.version === undefined ? {} : { cliVersion: chosen.version.version })
                })
              : createCodexExecCommand(executable, {
                  workspacePath: runCwd,
                  sandbox: effectiveSandbox,
                  ...choice
                })
        }
        /*
         * Outside Auto, Cursor asks before every connector call and a print
         * run has nobody to answer -- 17 "user rejected" in one Accept-edits
         * run (Colin's ledger, 2026-09-16). Auto passes `--force` and never
         * asks. Every other mode gets the answer written down where Cursor
         * reads it, before the run starts. Best effort: a workspace that
         * cannot be written leaves the run exactly as it was.
         *
         * NOT IN ASK OR PLAN (0.550). A wildcard per server lets every tool
         * of it run, writes included, and Cursor's own Ask is read-only Q&A,
         * not a connector filter (Cursor's research, 2026-10-02). So a
         * read-only run adds no rule: a connector tool runs there only if the
         * person allowed it themselves. Accept edits keeps the wildcard.
         *
         * TAKEN BACK when the run ends. The grant is made right before the
         * process starts (below, beside the default-model guard) so that the
         * one place a run ends -- its process completing, or failing to start
         * -- is the one place the rules are given back.
         */
        try {
          command = buildCommand(prompt)
        } catch (cause) {
          // The builder's own sentence is the reason; one line for every
          // reason hid "Cursor Agent takes no effort level" for a day.
          const why = cause instanceof Error && cause.message.length > 0 ? ` ${cause.message}.` : ''
          return error(
            'RUNTIME_START_FAILED',
            `That runtime cannot be started with the options chosen.${why} Nothing was recorded.`
          ) as CodexMissionStartResponse
        }

        /*
         * YOUR CLOUD COMPUTER, READIED (0.761): made or woken, OpenCode on it, this folder brought over -- each said
         * on the live line as it begins. Nothing is recorded until it is ready: a refusal here hands the words back.
         */
        let cloudSpawn: ServeSpawn | undefined
        /**
         * Where the agent stands there (0.762): the brief names this, as a worktree's run is told its own copy. Told
         * the folder's path here, a cloud agent went looking for `C:\Users\...` on Linux, failed a read, and said in
         * its reply that "the Windows path you gave maps to the workspace root here" (drive-a-cloud-turn, 0.761).
         */
        let cloudDir: string | undefined
        let cloudCodespace: string | undefined
        if (cloud !== undefined && options.cloud !== undefined) {
          const agent = runtime === 'claude' || runtime === 'codex' ? runtime : undefined
          const prepared = await options.cloud.prepare({
            missionId,
            folder: runCwd,
            ...(followUpOf === undefined ? {} : { follows: followUpOf }),
            openCodeVersion: chosen.version?.version,
            ...(agent === undefined ? {} : { agent: { runtime: agent, version: chosen.version?.version } }),
            ...(cloud.setUp === true ? { setUp: true } : {}),
            say
          })
          if (!prepared.ok) {
            if (prepared.signIn !== undefined) return { ok: false, error: { code: 'CLOUD_SIGN_IN_NEEDED', message: prepared.message, cloudSignIn: prepared.signIn } }
            return prepared.setup === undefined
              ? error('RUNTIME_START_FAILED', prepared.message) as CodexMissionStartResponse
              : { ok: false, error: { code: 'CLOUD_SETUP_NEEDED', message: prepared.message, cloudSetup: prepared.setup } }
          }
          if (startLifecycleVersion !== lifecycleVersion) {
            return error('RUNTIME_START_FAILED', 'The mission was stopped before your cloud computer was ready.') as CodexMissionStartResponse
          }
          cloudSpawn = prepared.spawn
          cloudDir = prepared.dir
          cloudCodespace = prepared.codespace
          // Its command again, now that it is known where it runs: the agent there, in the conversation's copy there.
          if (agent !== undefined) {
            if (prepared.bin === undefined) return error('RUNTIME_START_FAILED', 'Your cloud computer did not say where its copy of the agent is. Send again.') as CodexMissionStartResponse
            cloudLaunch = { executable: cloudLaunchOf(agent, prepared.bin), dir: prepared.dir }
            command = buildCommand(prompt)
          }
        }
        // The run's own folder when it is not this one: a teammate's worktree, or the copy on the cloud computer.
        const ownFolder = cloudDir ?? slot?.cwd

        // What the runtime is SENT is the person's words plus their teammates'
        // waiting messages and the share form. The ledger keeps the person's
        // words as the prompt and the delivered messages by id; the rest is
        // deterministic over those.
        // M16: attached files placed where this run reads, when it runs
        // anywhere but the project folder. Only what is sent changes.
        say('briefing')
        // 0.713: and an attached PDF opened for it, its text and pages beside it.
        const placed = await attachmentsPlacedForRun(prompt, runFolder, runCwd, options.readablePdf).catch(() => ({ prompt, files: [] as readonly string[] }))
        const attached = placed.prompt
        // On the cloud computer (0.762), the message's own files go to the copy there, which git never gave them. One
        // that does not arrive is left as named: the agent's own read says it is missing, as for a worktree (M16).
        if (cloudDir !== undefined && placed.files.length > 0) await options.cloud?.carry(missionId, runCwd, placed.files).catch(() => undefined)
        // Cold, not a command and not a forked side question: told what was said before (0.495).
        const sentPrompt = coldEarlier === undefined || bare || (side !== undefined && resumeThreadId !== undefined) ? attached : composeColdFollowUp(coldEarlier, attached, rewind === true)
        let runtimePrompt = sentPrompt
        let delivered: readonly WorkroomMessage[] = []
        let peerDeliveryFailed = false
        /*
         * A2.5: what the session this turn resumes was already told. Only a
         * turn that RESUMES one -- a cold start, a changed mode or another
         * runtime starts a session that holds nothing -- and never one that
         * compacted, whose record says more than the session now holds.
         */
        const earlierBrief =
          resumeThreadId !== undefined && resumedMissionId !== undefined && !resumedCompacted
            ? await options.briefSessions?.after(resumedMissionId).catch(() => undefined)
            : undefined
        const plan = briefPlan({ resumes: resumeThreadId !== undefined, compacted: resumedCompacted, earlier: earlierBrief })
        let briefGiven: readonly string[] | undefined
        // A command is sent bare, and records no brief: the session it acts
        // on may be cleared or compacted by it, so the next turn is briefed
        // whole (briefSessions.after finds nothing for this one).
        if (side !== undefined) {
          // The forked session already holds the brief and the whole
          // conversation; it is told only what this is -- and, when the
          // copy predates it, what the turn still running was asked (0.515).
          const running = sideRunningAsk === undefined
            ? ''
            : `\n\nThe conversation's latest turn is still running, and your copy of the conversation ends before it. That turn was asked:\n\n${sideRunningAsk.trim().length > SIDE_RUNNING_ASK_CHARS ? `${sideRunningAsk.trim().slice(0, SIDE_RUNNING_ASK_CHARS)}\u2026` : sideRunningAsk.trim()}`
          runtimePrompt = `${SIDE_QUESTION_PREFACE}${running}

${sentPrompt.trim()}`
        } else if (bare) {
          const named = openCodeCommand ?? codexCommand
          const after = named === undefined ? sentPrompt.trim() : sentPrompt.trimStart().slice(named.length + 1).trim()
          runtimePrompt = named === undefined
            ? after
            : codexCommand === 'init'
              ? [CODEX_INIT_PROMPT, after].filter((part) => part.length > 0).join('\n\n')
              : after
        } else if (peer !== undefined && peerExchange !== undefined) {
          // The turn this one continues, so the brief can find the
          // conversation's group. A route switch names it in `continuation`;
          // a follow-up in `resumedMissionId`; a first turn has none.
          const prepared = await peerExchange.prepare(sentPrompt, peer, runtime, {
            folder: runFolder,
            ...(slot?.instructions === undefined ? {} : { instructions: slot.instructions }),
            ...(ownFolder === undefined ? {} : { ownFolder }),
            ...((continuation?.missionId ?? resumedMissionId ?? followUpOf) === undefined
              ? {}
              : { previousMissionId: continuation?.missionId ?? resumedMissionId ?? followUpOf }),
            ...(plan.alreadyGiven === undefined ? {} : { alreadyGiven: plan.alreadyGiven }),
            // A2.12: what the host started this run to answer is shown to it.
            ...(relay?.answering === undefined ? {} : { startedFor: relay.answering }),
            // A2.9: files another teammate changed that this one changed too.
            ...((() => {
              const overlap = options.recentEdits?.overlapFor({ folder: runCwd, teammateId: peer.self.teammateId, now: now() })
              return overlap === undefined ? {} : { overlap }
            })())
          })
          runtimePrompt = prepared.runtimePrompt
          delivered = prepared.delivered
          peerDeliveryFailed = prepared.failed
          briefGiven = prepared.given
        } else if (peerExchange !== undefined && !sealed) {
          /*
           * A run that belongs to nobody still stands in the project folder
           * and still has the project's memory. It used to be briefed with
           * neither, because the whole briefing hung on a teammate (Grok,
           * pass 14, ranked second: the secret word answered NONE and no
           * memory file anywhere). Nothing here needs a roster.
           */
          const solo = await peerExchange.briefSolo(sentPrompt, runtime, {
            folder: runFolder,
            ...(ownFolder === undefined ? {} : { ownFolder }),
            ...((continuation?.missionId ?? resumedMissionId ?? followUpOf) === undefined
              ? {}
              : { previousMissionId: continuation?.missionId ?? resumedMissionId ?? followUpOf }),
            ...(plan.alreadyGiven === undefined ? {} : { alreadyGiven: plan.alreadyGiven })
          })
          runtimePrompt = solo.runtimePrompt
          briefGiven = solo.given
        }
        // Repeated even on a resumed session. Omitted references are never named in a brief that would imply access.
        if (!bare && options.references !== undefined) runtimePrompt = [runtimePrompt, referencesSection(referenceFolders)].join('\n\n')
        // A follow-up to a turn the person undid starts with what Undo put back (0.784, undone-note.ts).
        if (!bare && side === undefined && resumedRunId !== undefined) {
          const undone = (await options.turnRecords?.get(resumedRunId).catch(() => undefined))?.undone
          const told = undone === undefined ? undefined : undoneNote(undone)
          if (told !== undefined) runtimePrompt = `${told}\n\n${runtimePrompt}`
        }
        // Plan first, for a teammate's mission or a plain one. Appended last
        // so it is the instruction closest to the model's answer.
        //
        // The sandbox is checked as well as the mode, and the pair is what a
        // mutation guard holds: with Plan as a MODE the mapping above already
        // makes it read-only, so this condition cannot currently be half
        // true -- which is exactly why the guard targets the mapping instead
        // of this line. A "plan" that could edit the workspace is a promise
        // the app cannot keep, whichever of the two ever changes.
        // A read-only OpenCode run is told it has no shell, before the plan
        // instruction so that one stays last (A2.20, workroom-briefing.ts).
        // With Locust's git tool, OpenCode's own instructions say so (0.818).
        if (!bare && runtime === 'opencode' && sandbox === 'read-only' && openCodeGitReads === undefined) {
          runtimePrompt = [runtimePrompt, openCodeReadOnlySection()].join('\n\n')
        }
        if (!bare && mode === 'plan' && sandbox === 'read-only') {
          runtimePrompt = [runtimePrompt, planSection()].join('\n\n')
        }

        try {
          await options.ledger.createMission({
            missionId,
            runId,
            prompt,
            runtime,
            model: chosenModel ?? 'account-default',
            requestedRouteId: routeId,
            resolvedRouteId,
            ...(route.account === undefined ? {} : { account: route.account }),
            cliVersion: chosen.version?.version ?? null,
            workspaceId,
            sandbox: effectiveSandbox,
            // What was ASKED for, beside what it was allowed. `ask` and
            // `plan` are both read-only, so without this a plan reopened
            // after a restart came back as an ordinary read-only run and
            // lost its "Build this plan" offer (QA, 2026-09-06).
            mode,
            executionPolicyVersion: 1,
            createdAt,
            ...(continuation === undefined
              ? resumedMissionId === undefined
                ? {}
                : {
                    continuesFrom: {
                      missionId: resumedMissionId,
                      // Follow-ups do not reconcile: nothing stopped, so there
                      // is no checkpoint to point at. Epoch 1 is the mission's
                      // own first, which is the only one a reader could mean.
                      checkpointEpoch: 1,
                      reason: 'follow-up' as const,
                      // Absent when the turn being continued left no session:
                      // the record then says "this followed that" without
                      // claiming a resume that never happened.
                      ...(resumeThreadId === undefined ? {} : { runtimeThreadId: resumeThreadId }),
                      // An edit of an earlier message (0.498): the turn it replaces stays as the version before.
                      ...(rewind === true ? { edited: true as const } : {})
                    }
                  }
              : { continuesFrom: continuation }),
            // Who started this run. The relay starts a teammate's reply with a
            // prompt the HOST wrote, and until this was recorded the file said
            // only what every other mission says -- so the app had no way to
            // tell a conversation a person began from one it began itself, and
            // presented both the same way.
            ...(relay !== undefined
              ? { startedBy: { kind: 'relay' as const, hop: relay.hop } }
              : side !== undefined
                ? { startedBy: { kind: 'side' as const, of: side.of, question: side.question } }
              : startedBy === undefined
                ? {}
                : { startedBy }),
            // What was actually run, so a failure can be diagnosed from the
            // record instead of by reconstructing the command from the builder
            // and hoping the reconstruction matched -- which is how eight
            // experiments went on 2026-09-05.
            //
            // The command built HERE, not the one rebuilt below for runtimes
            // that take the prompt as an argument. They differ only in the
            // prompt text, and the prompt is replaced by a marker either way,
            // so the recorded value is identical -- and taking this one keeps
            // the briefing, which is longer and quotes teammates, out of it.
            command: recordableCommand(command, prompt)
          })
          if (references.length > 0 || referenceIds !== undefined) await options.references?.bind(missionId, continues, references.map((item) => item.id), runCwd)
          if (runtime === 'claude' && cloud === undefined) await options.claudeContext?.remember(missionId, {
            cwd: runCwd, executablePath: command.executablePath, args: command.args, sandbox: effectiveSandbox,
            ...(permissionBridge === undefined ? {} : { permission: permissionBridge.configPath }),
            ...(browserMcp === undefined ? {} : { browser: browserMcp.configPath }),
            plugins: skills?.plugins.map((plugin) => ({ name: plugin.name, dir: plugin.dir })) ?? []
          }).catch(() => undefined)
        } catch {
          return error(
            'PERSISTENCE_FAILED',
            'The mission could not be created in the durable local ledger.'
          ) as CodexMissionStartResponse
        }

        // What this mission's session now holds, for the turn that resumes
        // it (A2.5). Best effort: a record that is not written costs the next
        // turn a full brief, and nothing else.
        if (options.briefSessions !== undefined && briefGiven !== undefined) {
          await options.briefSessions
            .record(missionId, { given: briefHeld(plan.alreadyGiven, briefGiven), turns: plan.turns })
            .catch((cause: unknown) => {
              options.note?.('brief-record-failed', `${missionId}: ${cause instanceof Error ? cause.message : String(cause)}`)
            })
        }

        // Recorded BEFORE the process starts: a mission must not run on
        // messages its own record cannot name.
        if (peerExchange !== undefined && delivered.length > 0) {
          try {
            await peerExchange.recordReceived(missionId, delivered, createdAt)
          } catch {
            await options.ledger.appendHostFailure(missionId, {
              code: 'runtime-start-failed',
              message: 'The messages this mission was shown could not be recorded in the local ledger.',
              occurredAt: now().toISOString()
            }).catch(() => undefined)
            return error(
              'PERSISTENCE_FAILED',
              'The messages this mission was shown could not be recorded in the durable local ledger.'
            ) as CodexMissionStartResponse
          }
        }

        if (startLifecycleVersion !== lifecycleVersion) {
          try {
            await options.ledger.appendHostFailure(missionId, {
              code: 'runtime-start-failed',
              message: 'The Codex mission was stopped before launch.',
              occurredAt: now().toISOString()
            })
          } catch {
            return error(
              'PERSISTENCE_FAILED',
              'The stopped mission could not be finalized in the local ledger.'
            ) as CodexMissionStartResponse
          }
          return error(
            'RUNTIME_START_FAILED',
            'The Codex mission was stopped before launch.'
          ) as CodexMissionStartResponse
        }

        let diskBefore: WorkspaceSnapshot | undefined
        let checkpointBefore: Checkpoint | undefined
        let process: RuntimeProcessRun
        let steer: ((text: string) => Promise<boolean>) | undefined
        try {
          // The prompt the runtime is sent is the person's words plus their
          // teammates' messages; a runtime that takes it on the argv gets the
          // same text there. Options were validated above, so this cannot refuse.
          if (command.stdin === 'none') command = buildCommand(runtimePrompt)
          // Refused HERE rather than by cmd.exe, which answers a too-long
          // command line with exit 1, no output, and one line on stderr --
          // reported to the person as the runtime failing for no stated
          // reason. This is the last point at which the real length is known,
          // because the workroom briefing has just been folded in.
          const tooLong = commandTooLong(command)
          if (tooLong !== undefined) throw new Error(tooLong)
          // Look at the tree BEFORE the runtime can touch it. Only when it may:
          // a read-only run has nothing to observe, and asking git for every
          // question would be paying for an answer nobody reads.
          if (effectiveSandbox !== 'read-only') say('reading-folder')
          // And the folder as it stands, kept, so the turn can be undone (0.674). Only the folder itself: a
          // teammate's worktree or a comparison's copy has Keep and Discard for that.
          const keeps = effectiveSandbox !== 'read-only' && options.checkpoints !== undefined && runCwd === runFolder
          ;[diskBefore, checkpointBefore] = await Promise.all([
            effectiveSandbox === 'read-only' ? undefined : (options.observeDisk ?? snapshotWorkspace)(runCwd),
            keeps ? options.checkpoints!.take(runCwd) : undefined
          ])
          // Asked again once that look returns: git can take a while, and a
          // window closed or an app quit meanwhile must not get a run spawned
          // behind it (L5). The catch below records this as the reason.
          if (disposed || startLifecycleVersion !== lifecycleVersion) throw new Error('The Codex mission was stopped before launch.')
          if (codexStreams) {
            /*
             * "Starting <runtime>" from here on every route (0.699). Only the
             * plain-process route below said it, so Codex's app server,
             * OpenCode's server (every OpenCode mode since 0.677) and
             * Copilot's ACP went from "Reading the folder" -- done by then --
             * straight to "Working", and the start's note could not tell the
             * CLI's own boot from Locust's (drive-the-start-says-its-phase).
             */
            say('starting-runtime')
            // The policy is chosen here, not carried in the argv, because on
            // this transport it is JSON on a socket -- so the rule that only
            // a full-access mission may say `danger-full-access` is enforced
            // at the point it is chosen. See `codexAppServerPolicy`.
            const policy = codexAppServerPolicy(effectiveSandbox, mode)
            // Approve-each stops to ask, and this is who answers. Any other
            // mode never asks, and the run's default refuses just in case.
            const onRequest =
              mode === 'approve-each' && options.approvals !== undefined
                ? options.approvals.requestHandlerFor({ runId, missionId, cwd: runCwd, changesByItem })
                : undefined
            const streamed = startCodexAppServerRun({
              spawn: cloudDir !== undefined && cloudCodespace !== undefined
                ? (options.cloudAppServerSpawn ?? cloudAppServerSpawn)({ codespace: cloudCodespace, dir: cloudDir })
                : options.appServerSpawn!,
              command,
              prompt: runtimePrompt,
              sandbox: policy.sandbox,
              approvalPolicy: policy.approvalPolicy,
              ...(onRequest === undefined ? {} : { onRequest: async (asked) => {
                await flushBeforeApproval()
                return onRequest(asked)
              } }),
              ...(chosenModel === undefined ? {} : { model: chosenModel }),
              ...(route.effort === undefined ? {} : { effort: route.effort }),
              ...(resumeThreadId === undefined ? {} : { resumeThreadId }),
              ...(side === undefined ? {} : { forkThread: true }),
              ...(codexCommand === 'review' || codexCommand === 'compact' ? { slashCommand: codexCommand } : {}),
              ...(runSkills === undefined ? {} : { skillRoots: [runSkills.skillsDir] }),
              // Ask and Plan start none of the MCP servers the person's Codex runs as programs (0.824).
              ...(policy.sandbox === 'read-only' ? { threadConfigFromSettings: sealed ? codexSealedThreadConfig : codexReadOnlyThreadConfig } : {}),
              // No teammate ends your browser, Codex too (0.720).
              ...(() => {
                const codexThreadConfig: Record<string, JsonValue> = {
                  ...((codexGuardConfig as Readonly<Record<string, JsonValue>> | undefined) ?? {}),
                  ...(browserMcp === undefined
                    ? {}
                    : {
                        'mcp_servers.locust_browser.command': browserMcp.command,
                        'mcp_servers.locust_browser.args': [...browserMcp.args],
                        'mcp_servers.locust_browser.env': browserMcp.env,
                        'mcp_servers.locust_browser.enabled': true,
                        'mcp_servers.locust_browser.default_tools_approval_mode': 'approve'
                      })
                }
                return Object.keys(codexThreadConfig).length === 0 ? {} : { threadConfig: codexThreadConfig }
              })(),
              signal: controller.signal,
              now
            })
            process = streamed
            steer = streamed.steer
          } else if (opencodeAcp) {
            say('starting-runtime')
            const handler = mode === 'approve-each' ? options.approvals?.requestHandlerFor({ runId, missionId, cwd: runCwd, changesByItem, runtime: 'opencode' }) : undefined
            const acp = startOpenCodeAcpRun({
              spawn: options.acpSpawn!,
              executable,
              workspacePath: runCwd,
              launch: openCodeAcpLaunch!,
              prompt: runtimePrompt,
              ...(resumeThreadId === undefined ? {} : { resumeSessionId: resumeThreadId }),
              requiredConfig: {
                mode: OPENCODE_2_MODE,
                ...(chosenModel === undefined ? {} : { model: chosenModel }),
                ...(route.effort === undefined ? {} : { effort: route.effort })
              },
              ...(browserMcp === undefined ? {} : { mcpServers: [{
                name: 'locust_browser', command: browserMcp.command, args: browserMcp.args,
                env: Object.entries(browserMcp.env).map(([name, value]) => ({ name, value }))
              }] }),
              onCapabilities: (capabilities) => options.onAcpCapabilities?.('opencode', capabilities),
              onPermission: async (asked) => {
                if (handler === undefined) return effectiveSandbox === 'full-access' ? 'allow_once' : 'reject_once'
                await flushBeforeApproval()
                return acpAnswerFor(await handler(acpPermissionRequest(asked, runCwd)))
              },
              signal: controller.signal,
              now
            })
            process = acp
            steer = acp.steer
          } else if (opencodeServes) {
            say('starting-runtime')
            // Each thing OpenCode asks becomes the same card Codex's do, and
            // the person's answer goes back as the server's own reply.
            const handler = mode === 'approve-each' ? options.approvals?.requestHandlerFor({ runId, missionId, cwd: runCwd, changesByItem, runtime: 'opencode' }) : undefined
            process = startOpenCodeServeRun({
              // Auto approves what its config did not deny, as `run --auto` did (0.677).
              ...(mode !== 'approve-each' && effectiveSandbox === 'full-access' ? { approveAll: true } : {}),
              spawn: cloudSpawn ?? options.opencodeServeSpawn!,
              command,
              prompt: runtimePrompt,
              ...(openCodeCommand === undefined ? {} : { slashCommand: openCodeCommand }),
              ...(chosenModel === undefined ? {} : { model: chosenModel }),
              ...(route.effort === undefined ? {} : { variant: route.effort }),
              ...(resumeThreadId === undefined ? {} : { resumeSessionId: resumeThreadId }),
              ...(handler === undefined
                ? {}
                : { onPermission: async (asked) => {
                  await flushBeforeApproval()
                  return openCodeReplyFor(await handler(openCodePermissionRequest(asked, runCwd)))
                } }),
              // A refusal in Ask or Plan names the mode, so the reply says why and not "you declined" (0.822).
              ...(handler === undefined && effectiveSandbox === 'read-only' ? { refusal: openCodeReadOnlyRefusal(mode) } : {}),
              signal: controller.signal,
              now
            })
          } else if (ownAcp && acpAgentRun !== undefined) {
            say('starting-runtime')
            // Every action the agent asks to take is a card; nothing is answered for the person (0.777).
            const handler = options.approvals?.requestHandlerFor({ runId, missionId, cwd: runCwd, changesByItem, runtime: 'acp' })
            const modelOption = acpAgentRun.agent.answer?.modelOption
            const acp = startAcpRun({
              spawn: options.acpSpawn!,
              command,
              prompt: runtimePrompt,
              ...(resumeThreadId === undefined ? {} : { resumeSessionId: resumeThreadId }),
              ...(browserMcp === undefined
                ? {}
                : {
                    mcpServers: [
                      {
                        name: 'locust_browser',
                        command: browserMcp.command,
                        args: browserMcp.args,
                        env: Object.entries(browserMcp.env).map(([name, value]) => ({ name, value }))
                      }
                    ]
                  }),
              // The model picked for it, by the option its own Test named; nothing else of the agent's is changed.
              ...(acpAgentRun.model === undefined || modelOption === undefined ? {} : { requiredConfig: { [modelOption.id]: acpAgentRun.model } }),
              ...(handler === undefined
                ? {}
                : { onPermission: async (asked) => {
                  await flushBeforeApproval()
                  return acpAnswerFor(await handler(acpPermissionRequest(asked, runCwd)))
                } }),
              signal: controller.signal,
              now
            })
            process = acp
            steer = acp.steer
          } else if (copilotAcp) {
            say('starting-runtime')
            // Each thing Copilot asks becomes the same card Codex's do. A
            // denial's reason -- which ACP's answer has no room for -- and a
            // message to the busy teammate both reach it as its next prompt,
            // the moment this one ends (`steer`).
            const handler = options.approvals?.requestHandlerFor({ runId, missionId, cwd: runCwd, changesByItem, runtime: 'copilot' })
            const acp = startAcpRun({
              spawn: options.acpSpawn!,
              command,
              prompt: runtimePrompt,
              ...(resumeThreadId === undefined ? {} : { resumeSessionId: resumeThreadId }),
              ...(browserMcp === undefined
                ? {}
                : {
                    mcpServers: [
                      {
                        name: 'locust_browser',
                        command: browserMcp.command,
                        args: browserMcp.args,
                        env: Object.entries(browserMcp.env).map(([name, value]) => ({ name, value }))
                      }
                    ]
                  }),
              // Agent mode, which asks; never Autopilot, and allow-all off.
              modeId: COPILOT_ACP_SESSION.modeId,
              requiredConfig: COPILOT_ACP_SESSION.requiredConfig,
              ...(options.onAcpCapabilities === undefined ? {} : { onCapabilities: (capabilities: AcpCapabilities) => options.onAcpCapabilities?.('copilot', capabilities) }),
              ...(handler === undefined
                ? {}
                : { onPermission: async (asked) => {
                  await flushBeforeApproval()
                  return acpAnswerFor(await handler(acpPermissionRequest(asked, runCwd)))
                } }),
              signal: controller.signal,
              now
            })
            process = acp
            steer = acp.steer
          } else {
            // A model named to Cursor becomes the person's Cursor default;
            // it is put back when the run ends (0.431).
            const cursorGuard = runtime === 'cursor' && chosenModel !== undefined ? options.cursorDefaultModel : undefined
            if (cursorGuard !== undefined) await cursorGuard.before(chosenModel!)
            const connectorGrant =
              runtime === 'cursor' && effectiveSandbox === 'workspace-write' && options.allowConnectors !== undefined
                ? await options.allowConnectors(runCwd).catch(() => undefined)
                : undefined
            if (connectorGrant !== undefined && connectorGrant.added.length > 0) {
              options.note?.('connectors-allowed', `${missionId} ${connectorGrant.added.join(' ')}`)
            }
            try {
              // Claude Code there (0.768): the same runner, its process gh's ssh.
              const runner = cloudDir !== undefined && cloudCodespace !== undefined
                ? (options.cloudProcessRunner ?? ((target) => createNodeRuntimeProcessRunner({ spawnProcess: cloudProcessSpawn(target) })))({ codespace: cloudCodespace, dir: cloudDir })
                : options.runner
              say('starting-runtime')
              process = runner.start(command, runtimePrompt, { signal: controller.signal })
            } catch (startError) {
              if (skills !== undefined) void skills.dispose()
              if (cursorGuard !== undefined) void cursorGuard.after().catch(() => undefined)
              if (connectorGrant !== undefined) void connectorGrant.release().catch(() => undefined)
              throw startError
            }
            if (cursorGuard !== undefined) {
              const release = (): void => void cursorGuard.after().catch(() => undefined)
              process.completion.then(release, release)
            }
            if (connectorGrant !== undefined) {
              const giveBack = (): void => void connectorGrant.release().catch(() => undefined)
              process.completion.then(giveBack, giveBack)
            }
            if (skills !== undefined) {
              const clear = (): void => void skills.dispose()
              process.completion.then(clear, clear)
            }
            // A2.10: Claude Code's input stays open while its turn runs, so a
            // message can be handed to it there too.
            const send = process.send
            if (send !== undefined) steer = async (text) => send(text)
          }
        } catch (startError) {
          // A refusal this file raised knows WHY; anything else does not, and
          // must not borrow a specific-sounding reason it cannot back.
          const why = startError instanceof Error && startError.message.length > 0
            ? startError.message
            : 'The Codex process could not be started safely.'
          // A run that never started still registered a token. Let it go.
          void options.permissionHost?.release(runId).catch(() => undefined)
          void options.browserHost?.releaseRun(runId).catch(() => undefined)
          try {
            await options.ledger.appendHostFailure(missionId, {
              code: 'runtime-start-failed',
              message: why,
              occurredAt: now().toISOString()
            })
          } catch {
            return error(
              'PERSISTENCE_FAILED',
              'The failed mission could not be finalized in the local ledger.'
            ) as CodexMissionStartResponse
          }
          return error(
            'RUNTIME_START_FAILED',
            why
          ) as CodexMissionStartResponse
        }
        if (runSkills !== undefined) {
          // The run's copy of its skills goes when the run does, however it ends.
          const clear = (): void => void runSkills.dispose()
          process.completion.then(clear, clear)
          // Completion owns it now. Every earlier return or exception belongs to the start's finally.
          pendingRunSkills = undefined
        }
        if (cloudDir !== undefined) {
          // What the turn changed there is read once it ends, for the card under it (cloud-turns.ts), whichever agent.
          const settle = (): void => void options.cloud?.settled(missionId).catch(() => undefined)
          process.completion.then(settle, settle)
        }

        const mission: ActiveCodexMission = {
          runId,
          missionId,
          controller,
          normalizer,
          process,
          steer,
          emit,
          prompt,
          runtime,
          ...(route.account === undefined ? {} : { account: route.account }),
          peer,
          ...(slot === undefined ? {} : { slot: slot.key }),
          transcript: createTranscriptTracker(),
          sandbox: effectiveSandbox,
          model: chosenModel,
          relay,
          ...(startedBy === undefined ? {} : { startedBy }),
          diskBefore,
          checkpointBefore,
          cwd: runCwd,
          workspaceId,
          ...(continues === undefined ? {} : { continues }),
          sharedTree: false,
          settled: false,
          saidWhyAReadFailed: false,
          lastSequence: 0,
          startTiming: { sentAt, phaseAt },
          persisted: [],
          deliverMessages: deliverWhenRuntimeStarts(peerExchange, missionId, delivered)
        }
        active.set(runId, mission)
        /*
         * Before anything runs: has Cursor been told not to look at this
         * folder?
         *
         * Its file tools refuse an ignored path with "permission denied" and
         * no reason, so the model explains the failure by inventing a cause,
         * and the invention is what the person reads and then chases. Three
         * incidents on one machine (`shared/cursorIgnore.ts`), the last of
         * them a screenshot LOCUST had written into the workspace it then
         * handed over.
         *
         * Best effort, and never fatal: a run that can still use its shell
         * may do perfectly good work. What it cannot do is explain itself.
         */
        // Whoever else is writing in this same folder right now: neither of
        // us can be credited with what the tree looks like afterwards.
        if (mission.diskBefore !== undefined || mission.checkpointBefore !== undefined) {
          for (const other of active.values()) {
            if (other.runId === runId || other.cwd !== runCwd || (other.diskBefore === undefined && other.checkpointBefore === undefined)) continue
            // Still in `active` is not the same as still running. A run whose
            // process has exited is not sharing anything with this one, and
            // treating it as though it were is what put the notice on every
            // relayed reply.
            if (other.settled) continue
            other.sharedTree = true
            mission.sharedTree = true
          }
        }
        scheduleConsume(mission)

        return {
          ok: true,
          data: {
            runId,
            missionId,
            runtime,
            model: chosenModel ?? 'account-default',
            resolvedRouteId,
            ...(route.account === undefined ? {} : { account: route.account }),
            cliVersion: chosen.version?.version ?? null,
            sandbox: effectiveSandbox,
            mode,
            ...(mission.diskBefore !== undefined && mission.diskBefore.partial !== true && !mission.sharedTree ? { watchesDisk: true as const } : {}),
            peerMessages: delivered.map((message) => publicPeerMessage(message, 'received')),
            peerDeliveryFailed,
            ...(resumedMissionId === undefined
              ? {}
              : {
                  followsUp: {
                    missionId: resumedMissionId,
                    ...(resumeThreadId === undefined ? {} : { runtimeThreadId: resumeThreadId })
                  }
                })
          }
        }
      } finally {
        await pendingRunSkills?.dispose().catch(() => undefined)
        if (referenceRun !== undefined && !active.has(referenceRun)) await options.references?.release(referenceRun).catch(() => undefined)
        if (claimed) starting.delete(claimKey)
        contextStarting.delete(claimKey)
        startOperations.delete(startOperation)
        resolveStartOperation()
      }
    },

    cancel(runId: unknown): CodexMissionCancelResponse {
      const mission = validRunId(runId) ? active.get(runId) : undefined
      if (mission === undefined) {
        return error(
          'RUN_NOT_ACTIVE',
          'That Codex mission is no longer active.'
        ) as CodexMissionCancelResponse
      }
      // Refused BEFORE the abort, while the run's client is still alive, so
      // the runtime hears the refusal rather than only losing its pipe. The
      // same release happens again in `clearActive` for every other way a run
      // can end, and is idempotent; doing it only there would resolve the
      // pending promise after the client had been disposed, and the answer
      // would go nowhere. The old approval service released in this order too.
      options.approvals?.release(mission.runId)
      mission.controller.abort()
      return {
        ok: true,
        data: {
          runId: mission.runId,
          state: 'cancellation-requested'
        }
      }
    },

    decide(answer: MissionApprovalAnswer): boolean {
      return options.approvals?.decide(answer) ?? false
    },

    async previewSwitch(followUpOf: string, runtime: MissionRuntimeId, prompt: string, leaveOut?: readonly string[]): Promise<HandoffPreview> {
      const prior = await options.ledger.getMission(followUpOf).catch(() => undefined)
      if (prior === undefined) return { kind: 'refused', message: 'That conversation could not be read, so it cannot be continued on another runtime.' }
      if (prior.metadata.runtime === runtime) return { kind: 'same' }
      // The checkpoint a send would write, worked out and not written (reconcileMission is pure).
      const checkpoint = reconcileMission(
        { metadata: prior.metadata, events: prior.events, issues: prior.issues },
        { reason: 'route-switch', epoch: prior.checkpoints.length + 1, createdAt: new Date().toISOString() }
      )
      if (checkpoint.resumeSafety === 'unsafe') {
        return { kind: 'refused', message: `That conversation cannot be continued safely on another runtime: ${checkpoint.safetyReason}` }
      }
      const taskFile = longTaskFilePath(prior.metadata.prompt, prior.metadata.missionId)
      const composed = composeHandoffPrompt(prior.metadata.prompt, checkpoint, runtimeDisplayName(prior.metadata.runtime), prompt, await earlierTurnsOf(prior), taskFile, leaveOut, howTurnEnded(prior.events))
      if (composed === undefined) return { kind: 'refused', message: 'That conversation is too long to carry to another runtime with this reply.' }
      return {
        kind: 'switch',
        fromRuntime: runtimeDisplayName(prior.metadata.runtime),
        kept: composed.kept,
        omitted: composed.omitted,
        unsettledCount: checkpoint.unsettledActions.length,
        taskByFile: taskFile !== undefined,
        taskClipped: composed.taskClipped === true,
        leftOutByYou: composed.leftOutByYou
      }
    },

    async handOff(
      runId: unknown,
      runtime: MissionRuntimeId,
      mode: MissionMode,
      route: { readonly model?: string; readonly effort?: string; readonly account?: string },
      emit: (update: CodexMissionUpdate) => void
    ): Promise<MissionHandoffResponse> {
      const previous = validRunId(runId) ? active.get(runId) : undefined
      if (previous === undefined) {
        return error(
          'RUN_NOT_ACTIVE',
          'That mission is no longer active, so there is nothing to hand off.'
        ) as MissionHandoffResponse
      }
      // Refuse a handoff to the runtime already running it. Stopping a run to
      // restart it on the same route would cost the user their progress and
      // buy nothing, and it is much more likely to be a misclick than a wish.
      if (previous.runtime === runtime && previous.account === route.account) {
        return {
          ok: false,
          error: { code: 'HANDOFF_REFUSED', message: 'That mission is already on this runtime. Nothing was changed.', untouched: true }
        }
      }

      const fromMissionId = previous.missionId
      const fromRuntime = runtimeDisplayName(previous.runtime)
      const originalPrompt = previous.prompt

      // Asked before the run is stopped, so a refusal known in advance
      // costs nothing (M9).
      const refused = await targetRefusal(runtime, mode, route, previous.peer?.self.teammateId)
      if (refused !== undefined) {
        return {
          ok: false,
          error: { code: 'HANDOFF_REFUSED', message: `${refused} Nothing was stopped: the mission is still running on ${fromRuntime}.`, untouched: true }
        }
      }

      // Stop the run, then WAIT for its own records to settle before
      // reconciling. Checkpointing a still-draining mission would race the
      // consume loop and report actions as unsettled that were about to
      // report back -- the checkpoint would be pessimistic, and the briefing
      // would tell the next runtime to re-verify work that had finished.
      // THIS run's loop only. Other teammates' missions keep going, and a
      // handoff that waited for them would stall until strangers finished.
      previous.controller.abort()
      await settling.get(previous.runId)

      let checkpoint
      try {
        checkpoint = await options.ledger.createCheckpoint(fromMissionId, 'route-switch')
      } catch {
        return error(
          'HANDOFF_REFUSED',
          'The mission was stopped, but it could not be reconciled, so nothing was handed off.'
        ) as MissionHandoffResponse
      }

      // `unsafe` means the ledger itself is damaged: the checkpoint was not
      // even written. A briefing built from a record that cannot be trusted
      // would carry that damage into a fresh run under a confident heading.
      if (checkpoint.resumeSafety === 'unsafe') {
        return error(
          'HANDOFF_REFUSED',
          `The mission was stopped, but it could not be handed off safely: ${checkpoint.safetyReason}`
        ) as MissionHandoffResponse
      }

      // A long task by file, not refused (0.513, long-task-file.ts).
      const taskFile = await longTaskFile(originalPrompt, await projectFolderOf(options, fromMissionId), fromMissionId)
      const composed = composeHandoffPrompt(originalPrompt, checkpoint, fromRuntime, undefined, [], taskFile)
      const briefing = composed === undefined || taskFile === undefined ? composed : { ...composed, prompt: withAttachments(composed.prompt, [taskFile]) }
      if (briefing === undefined) {
        return error(
          'HANDOFF_REFUSED',
          'The mission was stopped, but its original request is too long to carry to another runtime.'
        ) as MissionHandoffResponse
      }

      // The continuation stays the same teammate's mission: it keeps the
      // roster, receives what is waiting, and shares under the same name.
      const started = await service.start(briefing.prompt, runtime, mode, route, emit, {
        missionId: fromMissionId,
        checkpointEpoch: checkpoint.epoch,
        reason: 'route-switch',
        // What the brief left out, kept so the divider can say it after a reopen (0.519).
        ...(briefing.omitted.length === 0 ? {} : { leftOut: briefing.omitted })
      }, previous.peer)
      if (!started.ok) {
        // Pass the start failure through unchanged but keep the stop visible:
        // the user asked for a switch and now has neither run.
        return {
          ok: false,
          error: {
            code: started.error.code,
            message: `The mission was stopped for the handoff, but the new run did not start. ${started.error.message}`
          }
        }
      }

      return {
        ok: true,
        data: {
          ...started.data,
          continuesFrom: { missionId: fromMissionId, checkpointEpoch: checkpoint.epoch },
          resumeSafety: checkpoint.resumeSafety,
          omittedBriefing: briefing.omitted,
          unsettledCount: checkpoint.unsettledActions.length
        }
      }
    },

    async resume(
      missionId: unknown,
      runtime: MissionRuntimeId,
      mode: MissionMode,
      route: { readonly model?: string; readonly effort?: string; readonly account?: string },
      emit: (update: CodexMissionUpdate) => void,
      peer?: MissionPeerContext
    ): Promise<MissionHandoffResponse> {
      if (!validRunId(missionId)) {
        return error('RUN_NOT_ACTIVE', 'That mission id is not one this app wrote.') as MissionHandoffResponse
      }
      // A mission still running has nothing to resume, and starting a second
      // run against the same work would have two processes writing the same
      // tree. Cancel it or let it finish first.
      if ([...active.values()].some((mission) => mission.missionId === missionId)) {
        return error(
          'HANDOFF_REFUSED',
          'That mission is still running, so there is nothing to resume.'
        ) as MissionHandoffResponse
      }

      let recovered
      try {
        recovered = await options.ledger.getMission(missionId)
      } catch {
        recovered = undefined
      }
      if (recovered === undefined) {
        return error(
          'HANDOFF_REFUSED',
          'That mission could not be read back from the local ledger, so there is nothing to continue from.'
        ) as MissionHandoffResponse
      }

      // H6: resumed on ANOTHER runtime, it is a route switch -- recorded as
      // one, so the thread draws the seam -- not a follow-up.
      const switching = runtime !== recovered.metadata.runtime || accountFromRoute(recovered.metadata.resolvedRouteId) !== route.account
      let checkpoint
      try {
        checkpoint = await options.ledger.createCheckpoint(missionId, switching ? 'route-switch' : 'manual')
      } catch {
        return error(
          'HANDOFF_REFUSED',
          'The mission could not be reconciled, so nothing was resumed.'
        ) as MissionHandoffResponse
      }

      // Same rule as a handoff, and for the same reason: `unsafe` means the
      // ledger itself is damaged, and a briefing built from a record that
      // cannot be trusted would carry that damage into a fresh run under a
      // confident heading. The renderer already declines to offer this, so
      // reaching here means the offer and the record disagreed -- which is
      // exactly when the host must not take the renderer's word for it.
      if (checkpoint.resumeSafety === 'unsafe') {
        return error(
          'HANDOFF_REFUSED',
          `This mission cannot be resumed: ${checkpoint.safetyReason}`
        ) as MissionHandoffResponse
      }

      // A long task by file, not refused (0.513, long-task-file.ts).
      const taskFile = await longTaskFile(recovered.metadata.prompt, await projectFolderOf(options, missionId), missionId)
      const composed = composeHandoffPrompt(
        recovered.metadata.prompt,
        checkpoint,
        runtimeDisplayName(recovered.metadata.runtime),
        undefined,
        [],
        taskFile,
        // A resumed run says how the one it continues ended (0.572).
        [],
        howTurnEnded(recovered.events)
      )
      const briefing = composed === undefined || taskFile === undefined ? composed : { ...composed, prompt: withAttachments(composed.prompt, [taskFile]) }
      if (briefing === undefined) {
        return error(
          'HANDOFF_REFUSED',
          'The original request is too long to carry into a resumed run.'
        ) as MissionHandoffResponse
      }

      const started = await service.start(
        briefing.prompt,
        runtime,
        mode,
        route,
        emit,
        // On the same runtime a resume is a follow-up -- a handoff divider
        // would claim a change of runtime that did not happen. On another,
        // it is exactly that change (H6).
        { missionId, checkpointEpoch: checkpoint.epoch, reason: switching ? 'route-switch' : 'follow-up' },
        peer,
        undefined,
        undefined,
        { kind: 'resume', epoch: checkpoint.epoch }
      )
      if (!started.ok) {
        return {
          ok: false,
          error: {
            code: started.error.code,
            message: `The mission could not be resumed. ${started.error.message}`
          }
        }
      }

      return {
        ok: true,
        data: {
          ...started.data,
          continuesFrom: { missionId, checkpointEpoch: checkpoint.epoch },
          resumeSafety: checkpoint.resumeSafety,
          omittedBriefing: briefing.omitted,
          unsettledCount: checkpoint.unsettledActions.length
        }
      }
    },

    accountInUse(account: string): boolean { return starting.size > 0 || [...active.values()].some(mission => mission.account === account) },
    hasMission(missionId: string): boolean {
      return [...active.values()].some((mission) => mission.missionId === missionId)
    },
    contextBusy(missionIds: readonly string[]): boolean {
      return [...contextStarting.values()].some((id) => missionIds.includes(id)) || [...active.values()].some((mission) => missionIds.includes(mission.missionId))
    },

    async steer(runId: string, text: string): Promise<boolean> {
      const mission = active.get(runId)
      if (mission?.steer === undefined) return false
      try {
        return await mission.steer(text)
      } catch {
        return false
      }
    },

    runIdOwnedBy(teammateId: string): string | undefined {
      return [...active.values()].find((mission) => ownerKeyOf(mission.peer, mission.slot) === teammateId)?.runId
    },

    liveMissionIds(): readonly string[] {
      return [...active.values()].map((mission) => mission.missionId)
    },

    interrupt(): void {
      lifecycleVersion += 1
      starting.clear()
      // Remember which missions the host cut short. `interrupt()` is
      // synchronous and the consume loops clear `active` once each aborted
      // process settles, so by the time `dispose()` can await a durable write
      // there is nothing left to name -- the ids are captured here or not at all.
      for (const mission of active.values()) {
        interruptedMissionIds.add(mission.missionId)
        mission.controller.abort()
      }
    },

    async dispose(): Promise<void> {
      disposed = true
      lifecycleVersion += 1
      starting.clear()
      for (const mission of active.values()) {
        interruptedMissionIds.add(mission.missionId)
        mission.controller.abort()
      }
      await Promise.allSettled([...startOperations, ...consumeOperations])
      for (const missionId of interruptedMissionIds) {
        try {
          // Reconcile AFTER each run's own records have settled, so the
          // checkpoint describes the finished ledger rather than racing it. On
          // the next launch this is what says which actions were left in doubt.
          await options.ledger.createCheckpoint(missionId, 'shutdown')
        } catch {
          // Best effort by design. A mission that cannot be checkpointed still
          // recovers from its events, and refusing to shut down over a
          // bookkeeping write would be a worse failure than the missing record.
        }
      }
    }
  }

  return service
}
