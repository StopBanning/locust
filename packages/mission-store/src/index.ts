import type {
  MissionRuntimeId,
  MissionSandbox,
  NormalizedRuntimeEvent,
  NormalizedRuntimeEventType,
  RedactedJsonValue
} from '@teammate/runtime-adapters'
import { constants as fsConstants } from 'node:fs'
import { mkdir, open, readdir, rename, stat, unlink, utimes } from 'node:fs/promises'
import type { FileHandle } from 'node:fs/promises'
import { isAbsolute, join } from 'node:path'

import { parsedCheckpoint, reconcileMission } from './checkpoint.js'
import { fitEventsToLedger, LEDGER_TEXT_LIMIT } from './fit.js'
import type { CheckpointReason, ReconciledCheckpoint } from './checkpoint.js'

/**
 * The version NEW ledger files are written at. Version 2 widened `runtime` from
 * the literal 'codex' to the mission-runtime union and `model` from the literal
 * 'account-default' to a free string, so a version-1 reader must not be handed
 * a version-2 file -- which is the entire reason the number moved.
 */
/**
 * What a person asked a run to do, as opposed to what it was allowed. Named
 * here rather than imported from the app: the ledger is read by things that
 * are not the app, and a record's vocabulary should not move when a UI does.
 */
export type MissionRecordedMode = "ask" | "plan" | "accept-edits" | "approve-each" | "auto";

export const MISSION_LEDGER_SCHEMA_VERSION = 27 as const

/**
 * Versions this reader accepts, each a strict subset of the next, so all are
 * read rather than rejected: bumping the number without this list would make
 * every mission recorded before the bump come back as `unsupported-schema`,
 * which reads to a user as their history disappearing. A file's version is
 * fixed by its header and every record in it must match, so appends to an
 * older mission stay at that mission's version.
 *
 * v1 -> v2 widened `runtime` and `model`. v2 -> v3 widened `sandbox` from the
 * literal 'read-only' to include 'workspace-write': a v2 reader must not be
 * handed a mission that was allowed to write, because it would render the
 * run's permissions as read-only and be wrong about what happened.
 *
 * v3 -> v4 adds `continuesFrom`. A handoff to a different runtime cannot be the
 * same mission: a mission records ONE runtime, and every event must agree with
 * it. So a handoff is a NEW mission that continues from a checkpoint of the
 * previous one -- which is also the truthful record, since two runs really did
 * happen. An older reader shown a v4 file would drop that link and present the
 * continuation as an unrelated mission.
 *
 * v4 -> v5 adds `mission.peer` records: a mission's cross-references into the
 * workroom, by message id, for the messages it was shown at its start and the
 * ones its work posted. The text lives in the workroom's own file, never here.
 * A v4 reader stops at the first record it cannot name, so a v5 file handed to
 * one would lose every record after the first peer link.
 *
 * v5 -> v6 widens `continuesFrom.reason` to include `follow-up`: a second turn
 * in the same conversation. It is a NEW mission for the same reason a route
 * switch is -- one mission holds one run, and a second turn is a second
 * process -- but it is not a handoff, and a reader that assumed every
 * continuation was a route switch would draw a handoff divider across an
 * ordinary reply.
 *
 * v6 -> v7 widens `runtime` again, to Cursor Agent and Gemini CLI. A v6 reader
 * handed a Gemini mission would refuse its header as invalid and report the
 * whole mission unreadable, so the number moves for the same reason it moved
 * from 1 to 2.
 *
 * v9 -> v10 adds `startedBy`. Until now every mission in the file looked like
 * something a person asked for, because every mission WAS. The relay broke
 * that: when one teammate writes to another, the host starts the recipient's
 * run by itself, with a prompt the host wrote. Nothing on the record said so,
 * so the app could only present that run as a mission the person began -- a
 * new conversation in their list, titled with machine instructions. Colin, on
 * 2026-09-04: *"this pops up as its own mission, 'missions' are like projects
 * or whole new conversations."*
 *
 * It is deliberately about WHO STARTED THE RUN rather than about the relay, so
 * a later host-started run has somewhere truthful to say so. An older reader
 * would drop the field and be wrong in exactly the way the app was.
 *
 * v11 -> v12 adds `command`: the executable and arguments the host actually
 * ran, with the prompt replaced by a marker. Everything else in this file
 * describes what a runtime SAID; this is the one record of what it was ASKED,
 * and its absence cost eight experiments on 2026-09-05 reconstructing a
 * command by reading the builder and hoping the reconstruction matched. For a
 * beta it matters more: a tester's machine cannot be borrowed, and a failure
 * report without the argv is a question nobody can answer remotely.
 *
 * v10 -> v11 takes that extension point up: `startedBy.kind` gains `resume`, a
 * run the host started to pick a mission back up from its last checkpoint
 * after the app stopped mid-work. Same reason as v10 -- its prompt is a
 * briefing the host wrote, so nothing may show it as a mission title -- and a
 * v10 reader would refuse the record outright, since `relay` was the only kind
 * it knew.
 *
 * v12 -> v13: `startedBy.kind` gains `routine`, a run the host started to
 * replay one step of a routine a person saved from an earlier conversation
 * (`routineId`, and which `step` this run is). Same rule as relay and resume:
 * the ledger says a person did not ask for this run, and a v12 reader would
 * refuse a starter it does not know rather than mislabel it.
 *
 * v14 -> v15 adds `mode`. The record has always said what a run was ALLOWED
 * (`sandbox`) and never what was ASKED FOR, and those are not the same
 * question: `ask` and `plan` are both read-only, so a plan reopened after a
 * restart came back as an ordinary read-only run -- its "Build this plan"
 * offer gone, and in its place a sentence about the change being only in the
 * reply, which is the wrong thing to say about a plan (QA, 2026-09-06). A
 * v14 reader shown a v15 file would drop the mode and be wrong in exactly
 * that way, quietly, so the number moves.
 *
 * v13 -> v14 widens `sandbox` again, to `full-access`: the Auto mode a person
 * switches on for themselves, in which a run is not confined to the workspace
 * folder. Exactly the reason the number moved from 2 to 3, one step further --
 * a v13 reader handed such a mission would refuse its header and report the
 * whole mission unreadable, and the alternative (letting it through under an
 * older number) would be worse, because that reader would draw a run that
 * could touch the whole machine as one confined to a folder. The permission a
 * run had is the last thing a record may be vague about.
 *
 * v16 -> v17 adds `mission.edit_check` records: what the person's own check
 * command said after a turn that changed files (A3.3). The host runs it after
 * the turn has ended, so it cannot be a runtime event -- nothing a runtime
 * said -- and without a record a reopened conversation lost the card and its
 * Send button. A v16 reader stops at the first record it cannot name, so the
 * number moves for the reason it moved at v5.
 *
 * v17 -> v18: `startedBy.kind` gains `terminal`, a turn the person had with
 * the runtime in its own terminal, on this conversation's session, brought
 * back into the record (docs/PLAN-TERMINAL-CATCH-UP-2026-09-27.md;
 * `exchange` counts them from 1). Locust did not run it -- no command, no
 * mode it chose -- and a v17 reader would refuse a starter it does not know
 * rather than draw it as a run Locust made, so the number moves for the
 * reason it moved at v11 and v13.
 *
 * v18 -> v19: `startedBy.kind` gains `side`, a question asked ON THE SIDE of
 * a conversation (docs/PLAN-2026-09-29-FOLDERS-LIKE-CLAUDE-CODE.md, Devin's
 * side chats): a read-only run on a FORK of that conversation's session, which
 * the conversation never sees. `of` names the conversation's turn it forked
 * from, `question` counts them from 1. A v18 reader would draw it as a turn of
 * no conversation, so the number moves for the reason it moved at v18.
 *
 * v19 -> v20 adds `mission.approval` records: a card the person answered, and
 * how (0.576). Saving a conversation's record (0.575) found the ledger held
 * no approval at all -- only a declined call's own words, and nothing of one
 * that was allowed -- so the record a person is told they get could not show
 * what they let a teammate do. The host answers the card, not the runtime, so
 * it cannot be a runtime event, and a v19 reader stops at a record it cannot
 * name: the number moves for the reason it moved at v5 and v17.
 *
 * v20 -> v21 adds a fourth answerer, `earlier-always` (0.616): a request the
 * person's own Always, pressed on an earlier card of the run, allowed with no
 * card (the PRD's R8, one decision path). Each host used to answer those
 * itself and the ledger held nothing of them. A v20 reader stops at an
 * approval whose answerer it does not know, and drops the rest of the turn
 * with it, so the number moves; a v20 mission cannot take one.
 *
 * v21 -> v22 adds `parentItemId` to a tool event: the call was made by a
 * helper the teammate sent out, not by the teammate (helper visibility,
 * 2026-10-05). A v21 reader keeps a tool event with a field it does not know,
 * so it would not stop -- it would draw every Read and Grep the helper made
 * as the teammate's own, and count them in the teammate's work. The number
 * moves so that reader refuses the file rather than misstate it. The field
 * is optional, so every older ledger reads as it did, with no child rows.
 */
// v23 records an Ask-only conversation started from another app through MCP.
// Earlier readers must refuse that origin rather than call it a person's own turn.
// v24 records a turn Locust started because the agent before it ran out of its plan (0.774, `carry-on`): nobody
// typed it. A v23 reader stops at a starter it does not know, so the number moves for the reason it moved at v23.
// v26 records a turn Locust started because the person's check failed after the turn before it and the person had
// said to send such a failure back (0.812, `check-fix`): nobody typed it either, so the number moves the same way.
// v27 records a mission on Kilo. A v26 reader has no such runtime.
export const SUPPORTED_MISSION_LEDGER_SCHEMA_VERSIONS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27] as const

export type MissionLedgerSchemaVersion =
  (typeof SUPPORTED_MISSION_LEDGER_SCHEMA_VERSIONS)[number]

/**
 * READ FROM THE LIST ABOVE, not written out again.
 *
 * This was sixteen `value === n` clauses, a hand-kept copy of the array it
 * sits under, and the two drifted the moment one of them moved: bumping to
 * v16 for Muse Code left this at 15, so the writer produced files its own
 * reader called `unsupported-schema` -- every mission written, none listed.
 * Caught by the suite, which is the one place this class of duplication
 * ever is.
 */
const SUPPORTED_VERSIONS: ReadonlySet<unknown> = new Set(SUPPORTED_MISSION_LEDGER_SCHEMA_VERSIONS)

function isSupportedSchemaVersion(value: unknown): value is MissionLedgerSchemaVersion {
  return SUPPORTED_VERSIONS.has(value)
}

const MAX_PROMPT_LENGTH = 8_000
// One number with the fitter, so what it shortens to is what this reads.
const MAX_TEXT_LENGTH = LEDGER_TEXT_LIMIT
const MAX_RECORD_BYTES = 512 * 1024
const MAX_APPEND_BYTES = 2 * 1024 * 1024
const MAX_LEDGER_BYTES = 64 * 1024 * 1024
/*
 * 2,000 since QA-2026-09-29 round 2, R28: a turn is a mission, and at 500 a
 * few weeks of steady use pushed older conversations out of the list.
 * Every file is stat'ed either way; the cap bounds what is READ.
 */
const MAX_MISSION_FILES = 2_000

/**
 * How many ledgers to hold open at once. Opening every file concurrently
 * exhausts the process's descriptors on a large history, and an `EMFILE`
 * is indistinguishable from a corrupt ledger to the code that reads it.
 */
const READ_CONCURRENCY = 16
const DEFAULT_MISSION_LIST_LIMIT = 20
const MAX_EVENTS_PER_APPEND = 100
const SAFE_ID = /^[A-Za-z0-9_-]{1,128}$/
const NORMALIZED_EVENT_TYPES = new Set<NormalizedRuntimeEventType>([
  'run.started',
  'plan.updated',
  'message.delta',
  'step.started',
  'step.completed',
  'step.failed',
  'tool.started',
  'tool.completed',
  'tool.failed',
  'route.limit_detected',
  'run.cancelled',
  'run.failed',
  'run.completed',
  'adapter.diagnostic'
])
const NO_FOLLOW = process.platform === 'win32' ? 0 : fsConstants.O_NOFOLLOW
const READ_FLAGS = fsConstants.O_RDONLY | NO_FOLLOW
const APPEND_FLAGS = fsConstants.O_WRONLY | fsConstants.O_APPEND | NO_FOLLOW

export interface MissionLedgerMetadata {
  readonly missionId: string
  readonly runId: string
  readonly prompt: string
  readonly runtime: MissionRuntimeId
  /** Route-specific model identifier, e.g. `account-default`. */
  readonly model: string
  readonly requestedRouteId: string
  readonly resolvedRouteId: string
  readonly cliVersion: string | null
  readonly workspaceId: string
  readonly sandbox: MissionSandbox
  /**
   * What was asked for, as opposed to what it was allowed. Absent on every
   * mission written before v15, which is why every reader treats it as
   * unknown rather than as a default.
   */
  readonly mode?: MissionRecordedMode
  readonly executionPolicyVersion: 1
  readonly createdAt: string
  /**
   * Set when this mission continues another after a route switch. Points at the
   * checkpoint it resumed from, so the pair can be shown as one piece of work
   * without pretending they were one run.
   */
  readonly continuesFrom?: MissionContinuation
  /**
   * Who started this run. Absent means a person did, which is every mission
   * written before v10 and most written after it.
   */
  readonly startedBy?: MissionStarter
  /**
   * The command the host ran. The prompt is NEVER here -- it is recorded once,
   * above, and a copy in the argv would both duplicate it and put it wherever
   * a "send me your ledger" request travels.
   */
  readonly command?: MissionCommand
}

export interface MissionCommand {
  readonly executablePath: string
  readonly args: readonly string[]
}

/**
 * A run nobody typed a prompt for.
 *
 * `relay` is one teammate answering another without a person in the loop: the
 * host wrote the prompt, so the words in `prompt` are instructions to a
 * runtime rather than anything a reader should be shown as a mission title.
 * `hop` is which automatic turn of the exchange this is, counting from 1.
 */
export type MissionStarter =
  | {
      readonly kind: 'mcp'
      /**
       * Handed over in a voice call (0.739): the call's own Codex used Locust's tools, as another app
       * would. Older readers ignore it and say "another app", which is still true.
       */
      readonly via?: 'voice'
    }
  | {
      readonly kind: 'relay'
      readonly hop: number
    }
  | {
      /** Picking a mission back up from a checkpoint after an interruption. */
      readonly kind: 'resume'
      /** The checkpoint epoch the new run continues from. */
      readonly epoch: number
    }
  | {
      /** One step of a routine a person saved from an earlier conversation, replayed by the host. */
      readonly kind: 'routine'
      readonly routineId: string
      /** Which step of the routine this run is, counting from 1. */
      readonly step: number
    }
  | {
      /**
       * A turn the person had with the runtime in its own terminal, on this
       * conversation's session, brought back into the record (v18). Locust
       * did not run it.
       */
      readonly kind: 'terminal'
      /** Which exchange of that terminal session this is, counting from 1. */
      readonly exchange: number
    }
  | {
      /**
       * A question asked on the side of a conversation (v19): a read-only run
       * on a fork of its session. The conversation itself never sees it.
       */
      readonly kind: 'side'
      /** The conversation's turn whose session was forked. */
      readonly of: string
      /** Which side question on that conversation this is, counting from 1. */
      readonly question: number
    }
  | {
      /**
       * The turn before ran out of its agent's plan, and Locust carried the work on with the agent the person
       * chose (v24, 0.774). Its prompt is Locust's, never the person's.
       */
      readonly kind: 'carry-on'
      /** The runtime whose plan ran out. */
      readonly from: MissionRuntimeId
    }
  | {
      /**
       * The turn before changed files, the person's check failed after it, and Locust sent the failure back to the
       * same teammate because the person turned that on for the folder (v26, 0.812). Its prompt is Locust's, never
       * the person's.
       */
      readonly kind: 'check-fix'
      /** Which fix this is, counting from 1. */
      readonly fix: number
      /** The most fixes the folder allowed when this one started. */
      readonly of: number
      /** The check, as the person set it. */
      readonly command: string
    }

/**
 * `route-switch` is a handoff to another runtime; `follow-up` is the next turn
 * of the same conversation with the same runtime. Both are new missions,
 * because a mission records one run.
 */
export type MissionContinuationReason = 'route-switch' | 'follow-up'

export interface MissionContinuation {
  readonly missionId: string
  readonly checkpointEpoch: number
  readonly reason: MissionContinuationReason
  /**
   * The runtime's own session handle, when the continuation resumed one. A
   * follow-up carries it so the record says which conversation was resumed
   * rather than leaving that to be inferred.
   */
  readonly runtimeThreadId?: string
  /**
   * This turn is an EDIT of an earlier message (0.498): the person went back
   * and changed the turn that first followed `missionId`, which still stands
   * as the version before. Optional and additive -- a build that does not know
   * it reads the turn as an ordinary follow-up -- so no schema version moves.
   */
  readonly edited?: true
  /**
   * What the handoff's brief left out to fit (0.519), by section name. The
   * divider says it after the switch; until this was recorded it said it
   * only while the window that made the switch was open. Optional and
   * additive, like `edited`: no schema version moves.
   */
  readonly leftOut?: readonly string[]
  /** What the person chose to leave out of the brief (0.527), by section name. Additive, like `leftOut`. */
  readonly leftOutByYou?: readonly string[]
}

export type MissionHostFailureCode = 'runtime-start-failed' | 'runtime-transport-failed'

/**
 * A mission's link to one workroom message. `received` means the message was
 * quoted into this mission's prompt as a claim from a teammate; `posted` means
 * this mission's work produced it. Only the id is held here: the message text
 * has exactly one home, the workroom, so two records can never disagree about
 * what was said.
 */
export interface MissionPeerLink {
  readonly direction: 'received' | 'posted'
  readonly messageId: string
  readonly peerTeammateId: string
  readonly occurredAt: string
}

/**
 * What the person's check command said after a turn that changed files. Only
 * the lines new since the folder's previous check are kept, bounded, never
 * the whole output: the output of a person's test suite is theirs, and
 * enormous, and the card only ever shows what is new.
 */
export interface MissionEditCheck {
  readonly command: string
  readonly outcome: 'passed' | 'failed' | 'timed-out' | 'could-not-run'
  readonly newLines: readonly string[]
  readonly unchanged: boolean
  readonly first: boolean
  readonly occurredAt: string
}

const MAX_EDIT_CHECK_LINES = 60
const MAX_EDIT_CHECK_LINE_LENGTH = 500

/**
 * A card the person answered (v20). What it asked, in the card's own words;
 * what the answer was; and who gave it -- the person on the card, the person
 * on the card while saving a rule ("Yes, and don't ask again"), a rule
 * they saved answering it before it reached them, or (v21) their own Always
 * on an earlier card of the run. `words` is what was said with it: the
 * person's reason for a denial, the rule's own sentence, or what the Always
 * covers.
 * A card nobody answered (the run ended first, or Locust refused it) is not
 * recorded here: the run's own events already say what happened to the call.
 */
export interface MissionApproval {
  readonly approvalId: string
  readonly kind: 'command' | 'file-change' | 'question' | 'connector'
  readonly asked: string
  readonly answer: 'allowed' | 'allowed-always' | 'denied' | 'answered'
  readonly by: 'card' | 'card-saving-a-rule' | 'saved-rule' | 'earlier-always'
  readonly words?: string
  readonly askedAt: string
  readonly occurredAt: string
}

const MAX_APPROVAL_ASKED = 2_000
const MAX_APPROVAL_WORDS = 1_000

export interface MissionHostFailure {
  readonly code: MissionHostFailureCode
  readonly message: string
  readonly occurredAt: string
}

export type RecoveredMissionPhase = 'completed' | 'failed' | 'cancelled' | 'interrupted'

export type MissionLedgerIssueCode =
  | 'file-limit-exceeded'
  | 'file-too-large'
  | 'read-failed'
  | 'truncated-tail'
  | 'invalid-record'
  | 'unsupported-schema'

export interface MissionLedgerIssue {
  readonly code: MissionLedgerIssueCode
  readonly missionId?: string
  readonly message: string
}

export interface RecoveredMission {
  readonly metadata: MissionLedgerMetadata
  readonly events: readonly NormalizedRuntimeEvent[]
  readonly hostFailures: readonly MissionHostFailure[]
  /** Reconciled checkpoints in ledger order; the last one is the newest. */
  readonly checkpoints: readonly ReconciledCheckpoint[]
  /** Workroom messages this mission received or posted, in ledger order. */
  readonly peerLinks: readonly MissionPeerLink[]
  /** What the person's check said after this turn, in ledger order (A3.3). */
  readonly editChecks: readonly MissionEditCheck[]
  /** The cards the person answered on this turn, and how, in ledger order (v20). */
  readonly approvals: readonly MissionApproval[]
  /** The file's schema version: from 20 on, a card answered on this turn is in `approvals`. */
  readonly schemaVersion?: MissionLedgerSchemaVersion
  readonly phase: RecoveredMissionPhase
  readonly lastUpdatedAt: string
  readonly ledgerSequence: number
  readonly issues: readonly MissionLedgerIssue[]
}

export interface MissionLedgerSnapshot {
  readonly missions: readonly RecoveredMission[]
  readonly issues: readonly MissionLedgerIssue[]
  /**
   * Ledger files that were read and produced NO mission.
   *
   * Counted here because here is the only place it is knowable. A caller
   * cannot derive it by comparing issue ids against `missions`: that list is
   * sliced to a page, so a mission that recovered perfectly well and merely
   * fell outside the page looks identical to one that could not be recovered
   * at all. Astra measured exactly that, 2026-09-09 -- an older incomplete
   * receipt, twenty newer clean missions, and a screen reading "20 local · 1
   * file could not be read" when every file had in fact been read.
   *
   * It is a count of FILES, not of issues: one truncated header raises both
   * `truncated-tail` and `invalid-record`.
   */
  readonly unreadableCount: number
}

export interface MissionLedger {
  createMission(metadata: MissionLedgerMetadata): Promise<void>
  appendEvents(missionId: string, events: readonly NormalizedRuntimeEvent[]): Promise<void>
  appendHostFailure(missionId: string, failure: MissionHostFailure): Promise<void>
  /**
   * Derive a checkpoint from what is durably recorded and append it.
   *
   * There is deliberately no `appendCheckpoint(checkpoint)`. A caller that
   * could hand in its own checkpoint could hand in one that disagrees with
   * the ledger -- and a checkpoint whose whole purpose is to be trusted at a
   * provider limit is worthless the moment it can be authored by the
   * component whose state is in doubt.
   *
   * The returned checkpoint is durable EXCEPT when it reports `unsafe`: a
   * ledger that already stops short during recovery cannot receive a record
   * anyone would read again, so that verdict is returned without being written.
   */
  createCheckpoint(missionId: string, reason: CheckpointReason): Promise<ReconciledCheckpoint>
  /** Record which workroom messages this mission was shown, or produced. */
  appendPeerLinks(missionId: string, links: readonly MissionPeerLink[]): Promise<void>
  /** Record what the person's check said after this turn (A3.3). */
  appendEditCheck(missionId: string, check: MissionEditCheck): Promise<void>
  /** A card the person answered (v20). A mission written before v20 cannot hold one and refuses it. */
  appendApproval(missionId: string, approval: MissionApproval): Promise<void>
  /**
   * Move a mission's record to the trash. Returns false when there was none.
   *
   * The mission leaves every listing immediately -- it is as gone as it ever
   * was from where a person is standing -- and the file itself is kept, byte
   * for byte, until `emptyTrash`. Nothing else is rewritten: other missions
   * that pointed at it (a continuation, a workroom message) keep their
   * pointers, which resolve to nothing while it is in the trash and resolve
   * again if it is restored. Readers already treat a missing link as "stop
   * here", so the truthful state is a thread that ends where the deleted
   * part began, not one that pretends the part never existed.
   *
   * Whether a LIVE mission may be deleted is not decided here; the store
   * cannot know what is running, and refuses nothing it is not in a position
   * to judge.
   */
  deleteMission(missionId: string): Promise<boolean>
  /** What is in the trash, newest deletion first. */
  listTrashedMissions(): Promise<readonly TrashedMission[]>
  /**
   * Put one back. False when it is not in the trash, or when a mission of
   * that id is live again -- a restore must never write over a record.
   */
  restoreMission(missionId: string): Promise<boolean>
  /** Delete the trash for good. Returns how many records went. */
  emptyTrash(): Promise<number>
  /**
   * What the local history costs and how far back it goes, so a person can
   * decide about it. Reads sizes, never contents.
   */
  storageReport(): Promise<MissionStorageReport>
  /**
   * Delete finished missions last updated before an instant.
   *
   * The store still refuses nothing it cannot judge: the caller names the
   * missions that are running (`protectMissionIds`), exactly as it does for
   * `deleteMission`. What the store DOES enforce is the thing only it can
   * see -- that pruning never breaks a conversation it is keeping.
   */
  pruneMissions(options: MissionPruneOptions): Promise<MissionPruneResult>
  getMission(missionId: string): Promise<RecoveredMission | undefined>
  listMissions(options?: MissionLedgerListOptions): Promise<MissionLedgerSnapshot>
  /**
   * The ledger files, newest first, by their size and modified time -- read
   * without opening any of them. With `readMission`, this lets a caller keep
   * what it parsed and read again only the files that changed: the whole
   * history was re-parsed on every run end, 490-560 ms on a real 138-mission
   * ledger, measured in the app (2026-09-22). Optional so a fake ledger in a
   * test need not grow it; a caller without it uses `listMissions`.
   */
  missionFiles?(): Promise<MissionFileListing>
  /** One mission file, parsed, with the issues reading it raised. */
  readMission?(missionId: string): Promise<MissionFileRead>
  flush(): Promise<void>
}

/** A ledger file's identity: when it last changed, and how big it is. */
export interface MissionFileStamp {
  readonly missionId: string
  readonly modifiedAt: number
  readonly size: number
}

export interface MissionFileListing {
  /** Newest first, capped the way `listMissions` caps what it reads. */
  readonly files: readonly MissionFileStamp[]
  /** Past the cap: said once, the way `listMissions` says it. */
  readonly issues: readonly MissionLedgerIssue[]
  /** Every ledger file found, before the cap. */
  readonly totalFiles?: number
}

export interface MissionFileRead {
  readonly mission?: RecoveredMission
  readonly issues: readonly MissionLedgerIssue[]
}

export interface MissionLedgerListOptions {
  readonly limit?: number
}

/** A mission that has been deleted and is being kept until the trash is emptied. */
export interface TrashedMission {
  readonly missionId: string
  /** When it was deleted, ISO. */
  readonly deletedAt: string
  readonly bytes: number
  /** The first line of what was asked, so a listing can name it. */
  readonly prompt?: string
  /** When the mission itself was created, ISO. */
  readonly createdAt?: string
}

export interface MissionStorageReport {
  readonly missionCount: number
  readonly byteTotal: number
  /** When the least recently updated mission was last written, if any. */
  readonly oldestUpdatedAt?: string
  /**
   * Files counted above whose contents could not be read. They are part of
   * the count and the bytes -- they are on disk -- but they have no date, and
   * a prune will not touch them. Stating the number keeps the other three
   * from describing a population they do not cover.
   */
  readonly unreadableCount: number
}

export interface MissionPruneOptions {
  /** Missions last updated strictly before this instant are candidates. */
  readonly before: string
  /**
   * Delete nothing outside this set. A confirmation passes the exact ids its
   * preview showed, so the deletion can only ever be NARROWER than what the
   * person agreed to -- never wider, whatever changed in between.
   */
  readonly only?: readonly string[]
  /**
   * Missions the caller knows are running. The store cannot see processes, so
   * it cannot judge this -- it only promises not to delete what it is told.
   */
  readonly protectMissionIds?: readonly string[]
  /**
   * Work out what would go without deleting anything. The preview a person is
   * shown has to come from the same code that does the deleting, or the
   * preview is a second implementation that can disagree with it -- and the
   * disagreement would only ever be discovered after the files were gone.
   */
  readonly dryRun?: boolean
}

export interface MissionPruneResult {
  readonly deleted: readonly string[]
  /**
   * Missions whose file could not be removed. Deleting is not all-or-nothing
   * on a real filesystem: a lock on one file must not discard the true record
   * of the ones already gone.
   */
  readonly failed: readonly string[]
  /**
   * Set when the prune refused to delete anything because some ledger could
   * not be read at all. An unreadable file may be the newest turn of an old
   * conversation, and deleting its parent would gut a conversation still in
   * use -- so nothing is deleted until it can be read.
   */
  readonly unreadable: readonly string[]
  /**
   * Old missions kept because a mission that SURVIVES continues from them.
   * Deleting these would leave a kept conversation missing its earlier turns,
   * which is a worse outcome than keeping a few old files.
   */
  readonly keptForContinuity: readonly string[]
  /** Old missions the caller named as running. */
  readonly keptAsRunning: readonly string[]
}

export interface FileMissionLedgerOptions {
  readonly rootDirectory: string
  /** Test seam. Checkpoints are stamped by the ledger, never by a caller. */
  readonly now?: () => Date
  /**
   * Test seam for the one destructive call. A locked or virus-scanned file
   * cannot be simulated portably, and the behaviour when unlink fails is
   * exactly the behaviour worth proving.
   */
  readonly removeFile?: (path: string) => Promise<void>
  /**
   * Test seam for the move into the trash, which is what deleting now is.
   * The case worth proving is the one `removeFile` exists for: a file a virus
   * scanner or sync client is holding cannot be moved either, and a bulk
   * delete has to report that mission as failed rather than deleted.
   */
  readonly moveFile?: (from: string, to: string) => Promise<void>
}

interface CreatedRecord {
  readonly schemaVersion: MissionLedgerSchemaVersion
  readonly recordType: 'mission.created'
  readonly ledgerSequence: 1
  readonly occurredAt: string
  readonly metadata: MissionLedgerMetadata
}

interface EventRecord {
  readonly schemaVersion: MissionLedgerSchemaVersion
  readonly recordType: 'mission.event'
  readonly ledgerSequence: number
  readonly occurredAt: string
  readonly event: NormalizedRuntimeEvent
}

interface HostFailureRecord {
  readonly schemaVersion: MissionLedgerSchemaVersion
  readonly recordType: 'mission.host_failure'
  readonly ledgerSequence: number
  readonly occurredAt: string
  readonly failure: MissionHostFailure
}

interface CheckpointRecord {
  readonly schemaVersion: MissionLedgerSchemaVersion
  readonly recordType: 'mission.checkpoint'
  readonly ledgerSequence: number
  readonly occurredAt: string
  readonly checkpoint: ReconciledCheckpoint
}

interface PeerRecord {
  readonly schemaVersion: MissionLedgerSchemaVersion
  readonly recordType: 'mission.peer'
  readonly ledgerSequence: number
  readonly occurredAt: string
  readonly link: MissionPeerLink
}

interface EditCheckRecord {
  readonly schemaVersion: MissionLedgerSchemaVersion
  readonly recordType: 'mission.edit_check'
  readonly ledgerSequence: number
  readonly occurredAt: string
  readonly check: MissionEditCheck
}

interface ApprovalRecord {
  readonly schemaVersion: MissionLedgerSchemaVersion
  readonly recordType: 'mission.approval'
  readonly ledgerSequence: number
  readonly occurredAt: string
  readonly approval: MissionApproval
}

type LedgerRecord = CreatedRecord | EventRecord | HostFailureRecord | CheckpointRecord | PeerRecord | EditCheckRecord | ApprovalRecord
type JsonObject = Record<string, unknown>

interface ParsedLedger {
  readonly mission?: RecoveredMission
  readonly issues: readonly MissionLedgerIssue[]
  readonly nextSequence?: number
  readonly nextEventSequence?: number
  readonly byteLength?: number
  /** The version this FILE is written at, which appends must not change. */
  readonly schemaVersion?: MissionLedgerSchemaVersion
}

function isObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function requireSafeId(value: string, label: string): string {
  if (!SAFE_ID.test(value)) throw new Error(`${label} is invalid`)
  return value
}

function requireText(value: string, label: string, maximum = MAX_TEXT_LENGTH): string {
  if (!value.trim() || value.length > maximum || value.includes('\0')) {
    throw new Error(`${label} is invalid`)
  }
  return value
}

function requireTimestamp(value: string, label: string): string {
  requireText(value, label, 128)
  if (!Number.isFinite(Date.parse(value))) throw new Error(`${label} is invalid`)
  return value
}

/**
 * Every runtime a mission may record. Widening this is a schema version:
 * v8 added opencode and copilot, v9 antigravity, v16 muse.
 *
 * IT IS A RECORD, NOT AN ARRAY, and that is the whole point.
 *
 * It was `readonly string[]`, so nothing tied it to `MissionRuntimeId` and
 * the compiler never asked. Adding Muse Code to the union, to the display
 * names, to the capability table and to the command builder therefore left
 * this behind -- and every Muse mission died at
 * **"Stopped -- the mission ledger could not be written"**, before the
 * runtime was ever launched. Colin hit it on 0.247.0, on the first run.
 *
 * `satisfies Record<MissionRuntimeId, true>` makes the next runtime a
 * compile error here rather than a refusal at the only moment it matters.
 * Anything a person can pick must be something the ledger can write down.
 */
const MISSION_RUNTIMES = {
  codex: true,
  claude: true,
  cursor: true,
  gemini: true,
  opencode: true,
  copilot: true,
  antigravity: true,
  muse: true,
  // Kilo Code's CLI (LEDGER v27).
  kilo: true,
  // An agent the person added, by its command line (0.777, LEDGER v25).
  acp: true
} as const satisfies Record<MissionRuntimeId, true>

function isMissionRuntime(value: unknown): value is MissionRuntimeId {
  return typeof value === 'string' && Object.hasOwn(MISSION_RUNTIMES, value)
}

function validateMetadata(metadata: MissionLedgerMetadata): MissionLedgerMetadata {
  requireSafeId(metadata.missionId, 'missionId')
  requireSafeId(metadata.runId, 'runId')
  requireText(metadata.prompt, 'prompt', MAX_PROMPT_LENGTH)
  requireText(metadata.requestedRouteId, 'requestedRouteId', 256)
  requireText(metadata.resolvedRouteId, 'resolvedRouteId', 256)
  if (!isMissionRuntime(metadata.runtime)) {
    throw new Error('Mission runtime metadata is invalid')
  }
  requireText(metadata.model, 'model', 256)
  if (metadata.cliVersion !== null) requireText(metadata.cliVersion, 'cliVersion', 256)
  requireSafeId(metadata.workspaceId, 'workspaceId')
  if (
    metadata.mode !== undefined
    && metadata.mode !== 'ask'
    && metadata.mode !== 'plan'
    && metadata.mode !== 'accept-edits'
    && metadata.mode !== 'approve-each'
    && metadata.mode !== 'auto'
  ) {
    throw new Error('Mission mode is invalid')
  }
  if (
    (metadata.sandbox !== 'read-only'
      && metadata.sandbox !== 'workspace-write'
      && metadata.sandbox !== 'full-access')
    || metadata.executionPolicyVersion !== 1
  ) {
    throw new Error('Mission execution policy is invalid')
  }
  requireTimestamp(metadata.createdAt, 'createdAt')
  if (metadata.continuesFrom !== undefined) {
    requireSafeId(metadata.continuesFrom.missionId, 'continuesFrom.missionId')
    if (
      !Number.isSafeInteger(metadata.continuesFrom.checkpointEpoch)
      || metadata.continuesFrom.checkpointEpoch < 1
      || (metadata.continuesFrom.reason !== 'route-switch' && metadata.continuesFrom.reason !== 'follow-up')
    ) {
      throw new Error('Mission continuation is invalid')
    }
    if (metadata.continuesFrom.runtimeThreadId !== undefined) {
      requireText(metadata.continuesFrom.runtimeThreadId, 'continuesFrom.runtimeThreadId', 2_048)
    }
    if (metadata.continuesFrom.edited !== undefined && metadata.continuesFrom.edited !== true) {
      throw new Error('Mission continuation is invalid')
    }
    if (metadata.continuesFrom.leftOut !== undefined) {
      if (!Array.isArray(metadata.continuesFrom.leftOut) || metadata.continuesFrom.leftOut.length > 8) throw new Error('Mission continuation is invalid')
      for (const name of metadata.continuesFrom.leftOut) requireText(name, 'continuesFrom.leftOut', 40)
    }
    if (metadata.continuesFrom.leftOutByYou !== undefined) {
      if (!Array.isArray(metadata.continuesFrom.leftOutByYou) || metadata.continuesFrom.leftOutByYou.length > 8) throw new Error('Mission continuation is invalid')
      for (const name of metadata.continuesFrom.leftOutByYou) requireText(name, 'continuesFrom.leftOutByYou', 40)
    }
  }
  if (metadata.command !== undefined) {
    requireText(metadata.command.executablePath, 'command.executablePath', 1_024)
    if (!Array.isArray(metadata.command.args) || metadata.command.args.length > 128) {
      throw new Error('Mission command is invalid')
    }
    for (const argument of metadata.command.args) {
      // Every argument is checked, not just the shape: an unbounded or
      // control-carrying value here would ride into every reader of the file.
      requireText(argument, 'command.args entry', 1_024)
    }
  }
  if (metadata.startedBy !== undefined) {
    const starter = metadata.startedBy
    const counter =
      starter.kind === 'relay'
        ? starter.hop
        : starter.kind === 'resume'
          ? starter.epoch
          : starter.kind === 'routine'
            ? starter.step
            : starter.kind === 'terminal'
              ? starter.exchange
              : starter.kind === 'side'
                ? starter.question
                : starter.kind === 'check-fix'
                  ? starter.fix
                  : undefined
    if (
      (starter.kind !== 'relay' && starter.kind !== 'resume' && starter.kind !== 'routine' && starter.kind !== 'terminal' && starter.kind !== 'side' && starter.kind !== 'mcp' && starter.kind !== 'carry-on' && starter.kind !== 'check-fix')
      || (starter.kind !== 'mcp' && starter.kind !== 'carry-on' && (counter === undefined || !Number.isSafeInteger(counter) || counter < 1))
    ) {
      throw new Error('Mission starter is invalid')
    }
    if (starter.kind === 'carry-on' && !isMissionRuntime(starter.from)) throw new Error('Mission starter is invalid')
    // A fix is one of at most five, and never past the most the folder allowed.
    if (starter.kind === 'check-fix') {
      if (!Number.isSafeInteger(starter.of) || starter.of < starter.fix || starter.of > 5) throw new Error('Mission starter is invalid')
      requireText(starter.command, 'startedBy.command', 500)
    }
    if (starter.kind === 'routine') requireText(starter.routineId, 'startedBy.routineId', 200)
    if (starter.kind === 'side') requireSafeId(starter.of, 'startedBy.of')
  }
  return metadata
}

function validateFailure(failure: MissionHostFailure): MissionHostFailure {
  if (failure.code !== 'runtime-start-failed' && failure.code !== 'runtime-transport-failed') {
    throw new Error('Host failure code is invalid')
  }
  requireText(failure.message, 'Host failure message', 1_024)
  requireTimestamp(failure.occurredAt, 'Host failure timestamp')
  return failure
}

function validatePeerLink(link: MissionPeerLink): MissionPeerLink {
  if (link.direction !== 'received' && link.direction !== 'posted') {
    throw new Error('Peer link direction is invalid')
  }
  requireSafeId(link.messageId, 'messageId')
  requireSafeId(link.peerTeammateId, 'peerTeammateId')
  requireTimestamp(link.occurredAt, 'Peer link timestamp')
  return link
}

function validateEditCheck(check: MissionEditCheck): MissionEditCheck {
  if (!['passed', 'failed', 'timed-out', 'could-not-run'].includes(check.outcome)) {
    throw new Error('Edit check outcome is invalid')
  }
  requireText(check.command, 'Edit check command', 500)
  if (!Array.isArray(check.newLines) || check.newLines.length > MAX_EDIT_CHECK_LINES) {
    throw new Error('Edit check lines are invalid')
  }
  for (const line of check.newLines) {
    if (typeof line !== 'string' || line.length > MAX_EDIT_CHECK_LINE_LENGTH) throw new Error('Edit check line is invalid')
  }
  if (typeof check.unchanged !== 'boolean' || typeof check.first !== 'boolean') {
    throw new Error('Edit check flags are invalid')
  }
  requireTimestamp(check.occurredAt, 'Edit check timestamp')
  return check
}

/**
 * The host's own check result, cut to what the record can hold: the first
 * lines, each at most a line's worth. Exported so the host bounds it exactly
 * the way the reader will accept it.
 */
export function boundedEditCheck(check: MissionEditCheck): MissionEditCheck {
  return {
    command: check.command.slice(0, 500),
    outcome: check.outcome,
    newLines: check.newLines.slice(0, MAX_EDIT_CHECK_LINES).map((line) => line.slice(0, MAX_EDIT_CHECK_LINE_LENGTH)),
    unchanged: check.unchanged,
    first: check.first,
    occurredAt: check.occurredAt
  }
}

function validateApproval(approval: MissionApproval): MissionApproval {
  requireText(approval.approvalId, 'Approval id', 200)
  if (!['command', 'file-change', 'question', 'connector'].includes(approval.kind)) throw new Error('Approval kind is invalid')
  requireText(approval.asked, 'Approval question', MAX_APPROVAL_ASKED)
  if (!['allowed', 'allowed-always', 'denied', 'answered'].includes(approval.answer)) throw new Error('Approval answer is invalid')
  if (!['card', 'card-saving-a-rule', 'saved-rule', 'earlier-always'].includes(approval.by)) throw new Error('Approval answerer is invalid')
  if (approval.words !== undefined && (typeof approval.words !== 'string' || approval.words.length > MAX_APPROVAL_WORDS)) {
    throw new Error('Approval words are invalid')
  }
  requireTimestamp(approval.askedAt, 'Approval asked timestamp')
  requireTimestamp(approval.occurredAt, 'Approval timestamp')
  return approval
}

/** A card's answer, cut to what the record can hold. Exported so the host bounds it the way the reader accepts it. */
export function boundedApproval(approval: MissionApproval): MissionApproval {
  const asked = approval.asked.trim().slice(0, MAX_APPROVAL_ASKED)
  const words = approval.words?.trim().slice(0, MAX_APPROVAL_WORDS)
  return {
    approvalId: approval.approvalId.slice(0, 200),
    kind: approval.kind,
    asked: asked.length === 0 ? '(the card said nothing more)' : asked,
    answer: approval.answer,
    by: approval.by,
    ...(words === undefined || words.length === 0 ? {} : { words }),
    askedAt: approval.askedAt,
    occurredAt: approval.occurredAt
  }
}

function parsedApproval(value: unknown, schemaVersion: number): MissionApproval | undefined {
  if (!isObject(value)) return undefined
  try {
    const candidate = value as unknown as MissionApproval
    validateApproval(candidate)
    // A v20 file never held this answerer; one claiming it was not written by a v20 writer.
    if (candidate.by === 'earlier-always' && schemaVersion < 21) return undefined
    return {
      approvalId: candidate.approvalId,
      kind: candidate.kind,
      asked: candidate.asked,
      answer: candidate.answer,
      by: candidate.by,
      ...(candidate.words === undefined ? {} : { words: candidate.words }),
      askedAt: candidate.askedAt,
      occurredAt: candidate.occurredAt
    }
  } catch {
    return undefined
  }
}

function parsedEditCheck(value: unknown): MissionEditCheck | undefined {
  if (!isObject(value)) return undefined
  try {
    const candidate = value as unknown as MissionEditCheck
    validateEditCheck(candidate)
    return {
      command: candidate.command,
      outcome: candidate.outcome,
      newLines: [...candidate.newLines],
      unchanged: candidate.unchanged,
      first: candidate.first,
      occurredAt: candidate.occurredAt
    }
  } catch {
    return undefined
  }
}

function parsedPeerLink(value: unknown): MissionPeerLink | undefined {
  if (!isObject(value)) return undefined
  const candidate = value as Partial<MissionPeerLink>
  if (
    (candidate.direction !== 'received' && candidate.direction !== 'posted')
    || typeof candidate.messageId !== 'string'
    || typeof candidate.peerTeammateId !== 'string'
    || typeof candidate.occurredAt !== 'string'
  ) return undefined
  try {
    return validatePeerLink(candidate as MissionPeerLink)
  } catch {
    return undefined
  }
}

function recordLine(record: LedgerRecord): string {
  const line = `${JSON.stringify(record)}\n`
  if (Buffer.byteLength(line, 'utf8') > MAX_RECORD_BYTES) {
    throw new Error('Mission ledger record exceeds its safety limit')
  }
  return line
}

function missionPath(rootDirectory: string, missionId: string): string {
  return join(rootDirectory, `${requireSafeId(missionId, 'missionId')}.jsonl`)
}

/**
 * Where a deleted mission waits.
 *
 * A DIRECTORY inside the ledger, which is what makes this safe to add: every
 * reader here lists with `entry.isFile() && entry.name.endsWith('.jsonl')`,
 * so a folder is skipped by all of them without one of them being changed.
 * A trashed mission is therefore invisible to `listMissions`,
 * `storageReport` and `pruneMissions` while its bytes stay exactly as they
 * were.
 */
export const MISSION_TRASH_DIR = '.trash'

function trashPath(rootDirectory: string, missionId: string): string {
  return join(rootDirectory, MISSION_TRASH_DIR, `${requireSafeId(missionId, 'missionId')}.jsonl`)
}

function publicIssue(
  code: MissionLedgerIssueCode,
  message: string,
  missionId?: string
): MissionLedgerIssue {
  return {
    code,
    ...(missionId === undefined ? {} : { missionId }),
    message
  }
}

function parsedMetadata(
  value: unknown,
  schemaVersion: MissionLedgerSchemaVersion
): MissionLedgerMetadata | undefined {
  if (!isObject(value)) return undefined
  const candidate = value as Partial<MissionLedgerMetadata>
  // A version-1 file could only ever hold these two values. Accepting anything
  // wider here would let a hand-edited v1 file describe a runtime that version
  // of the writer could not have produced.
  if (schemaVersion === 1 && (candidate.runtime !== 'codex' || candidate.model !== 'account-default')) {
    return undefined
  }
  // Versions before 3 could only ever record a read-only mission, so a file
  // claiming otherwise was hand-edited and is refused rather than believed.
  if (schemaVersion < 3 && candidate.sandbox !== 'read-only') {
    return undefined
  }
  // Same rule for continuation: no writer before v4 could produce one.
  if (schemaVersion < 4 && candidate.continuesFrom !== undefined) {
    return undefined
  }
  // And no writer before v6 could call one a follow-up.
  if (schemaVersion < 6 && candidate.continuesFrom?.reason === 'follow-up') {
    return undefined
  }
  // And no writer before v10 could say a run was started by anything but a
  // person, so a file claiming otherwise was hand-edited.
  if (schemaVersion < 10 && candidate.startedBy !== undefined) {
    return undefined
  }
  // And no writer before v12 recorded the command it ran.
  if (schemaVersion < 12 && candidate.command !== undefined) {
    return undefined
  }
  // And no writer before v11 knew any starter but the relay.
  if (schemaVersion < 11 && candidate.startedBy !== undefined && candidate.startedBy.kind !== 'relay') {
    return undefined
  }
  // And no writer before v13 knew the routine starter.

  if (schemaVersion < 15 && candidate.mode !== undefined) {
    return undefined
  }
  if (schemaVersion < 14 && candidate.sandbox === 'full-access') {
    return undefined
  }
  if (schemaVersion < 13 && candidate.startedBy?.kind === 'routine') {
    return undefined
  }
  // And no writer before v18 brought a terminal's turns back.
  if (schemaVersion < 18 && candidate.startedBy?.kind === 'terminal') {
    return undefined
  }
  // And no writer before v19 asked anything on the side.
  if (schemaVersion < 19 && candidate.startedBy?.kind === 'side') {
    return undefined
  }
  if (schemaVersion < 23 && candidate.startedBy?.kind === 'mcp') return undefined
  // And no writer before v24 carried a turn on after a plan ran out.
  if (schemaVersion < 24 && candidate.startedBy?.kind === 'carry-on') return undefined
  // And no writer before v26 sent a failing check back to fix (0.812).
  if (schemaVersion < 26 && candidate.startedBy?.kind === 'check-fix') return undefined
  // And no writer before v25 ran an agent the person added (0.777).
  if (schemaVersion < 25 && candidate.runtime === 'acp') return undefined
  // And no writer before v7 knew Cursor Agent or Gemini CLI.
  if (schemaVersion < 7 && candidate.runtime !== 'codex' && candidate.runtime !== 'claude') {
    return undefined
  }
  // And no writer before v16 knew Muse Code.
  if (schemaVersion < 16 && candidate.runtime === 'muse') {
    return undefined
  }
  // And no writer before v27 knew Kilo.
  if (schemaVersion < 27 && candidate.runtime === 'kilo') {
    return undefined
  }
  if (
    typeof candidate.missionId !== 'string'
    || typeof candidate.runId !== 'string'
    || typeof candidate.prompt !== 'string'
    || !isMissionRuntime(candidate.runtime)
    || typeof candidate.model !== 'string'
    || typeof candidate.requestedRouteId !== 'string'
    || typeof candidate.resolvedRouteId !== 'string'
    || !(candidate.cliVersion === null || typeof candidate.cliVersion === 'string')
    || typeof candidate.workspaceId !== 'string'
    || (candidate.sandbox !== 'read-only'
      && candidate.sandbox !== 'workspace-write'
      && candidate.sandbox !== 'full-access')
    || candidate.executionPolicyVersion !== 1
    || typeof candidate.createdAt !== 'string'
  ) return undefined
  try {
    return validateMetadata(candidate as MissionLedgerMetadata)
  } catch {
    return undefined
  }
}

function isText(value: unknown, maximum = MAX_TEXT_LENGTH): value is string {
  return typeof value === 'string' && value.length <= maximum && !value.includes('\0')
}

function isNonemptyText(value: unknown, maximum = MAX_TEXT_LENGTH): value is string {
  return isText(value, maximum) && value.trim().length > 0
}

function isTimestamp(value: unknown): value is string {
  return typeof value === 'string'
    && value.length <= 128
    && !value.includes('\0')
    && Number.isFinite(Date.parse(value))
}

function isOptionalText(value: unknown, maximum = MAX_TEXT_LENGTH): boolean {
  return value === undefined || isText(value, maximum)
}

function isRedactedJson(value: unknown, depth = 0): value is RedactedJsonValue {
  if (depth > 8) return false
  if (value === null || typeof value === 'boolean') return true
  if (typeof value === 'number') return Number.isFinite(value)
  if (typeof value === 'string') return value.length <= MAX_TEXT_LENGTH && !value.includes('\0')
  if (Array.isArray(value)) {
    return value.length <= 100 && value.every((entry) => isRedactedJson(entry, depth + 1))
  }
  if (!isObject(value)) return false
  const entries = Object.entries(value)
  return entries.length <= 100
    && entries.every(([key, entry]) => isText(key, 512) && isRedactedJson(entry, depth + 1))
}

function isEvidence(value: unknown): boolean {
  if (!isObject(value) || typeof value.redacted !== 'boolean') return false
  if (value.transportSequence !== undefined
    && (!Number.isSafeInteger(value.transportSequence) || (value.transportSequence as number) < 1)) return false
  if (!isOptionalText(value.runtimeEventType, 512)) return false
  return value.raw === undefined || isRedactedJson(value.raw)
}

function isProcessEvidence(value: unknown): boolean {
  if (!isObject(value)) return false
  return (value.exitCode === null || Number.isSafeInteger(value.exitCode))
    && (value.signal === null || isNonemptyText(value.signal, 64))
    && isText(value.stderr, MAX_RECORD_BYTES)
    && typeof value.stderrTruncated === 'boolean'
    && Number.isSafeInteger(value.recordCount)
    && (value.recordCount as number) >= 0
    && typeof value.inputDeliveryFailed === 'boolean'
    && typeof value.outputLimitExceeded === 'boolean'
    && typeof value.forcedTerminationAttempted === 'boolean'
    && typeof value.terminationUnconfirmed === 'boolean'
    && isTimestamp(value.startedAt)
    && isTimestamp(value.finishedAt)
}

function isStepPayload(value: JsonObject): boolean {
  return (value.stepKind === 'turn' || value.stepKind === 'reasoning' || value.stepKind === 'item')
    && isOptionalText(value.itemId, 512)
    && isOptionalText(value.itemType, 512)
    && isOptionalText(value.status, 512)
    && isOptionalText(value.message)
    && isEvidence(value.evidence)
}

function isToolPayload(value: JsonObject): boolean {
  return isNonemptyText(value.itemId, 512)
    && isNonemptyText(value.toolKind, 512)
    && isNonemptyText(value.name, 512)
    && isOptionalText(value.command)
    // What the model said it was doing, when the runtime carries one. Claude
    // Code's Bash tool takes a description on every call and it is the whole
    // reason its own transcript reads in sentences. Optional, so every record
    // written before this stays valid.
    && isOptionalText(value.title)
    // The helper that made the call (v22), by the item id of its own row.
    && (value.parentItemId === undefined || isNonemptyText(value.parentItemId, 512))
    && (value.output === undefined || isRedactedJson(value.output))
    && (value.exitCode === undefined || Number.isSafeInteger(value.exitCode))
    && isOptionalText(value.status, 512)
    && (value.phase === 'started' || value.phase === 'updated' || value.phase === 'completed')
    && isEvidence(value.evidence)
}

function isEventPayload(type: NormalizedRuntimeEventType, value: JsonObject): boolean {
  if (type === 'run.started') {
    return isNonemptyText(value.runtimeThreadId, 2_048) && isEvidence(value.evidence)
  }
  if (type === 'plan.updated') {
    return isNonemptyText(value.itemId, 512)
      && isRedactedJson(value.plan)
      && typeof value.final === 'boolean'
      && isEvidence(value.evidence)
  }
  if (type === 'message.delta') {
    return isNonemptyText(value.itemId, 512)
      && (value.operation === 'append' || value.operation === 'replace')
      && isText(value.text)
      && typeof value.final === 'boolean'
      && isEvidence(value.evidence)
  }
  if (type === 'step.started' || type === 'step.completed' || type === 'step.failed') {
    return isStepPayload(value)
  }
  if (type === 'tool.started' || type === 'tool.completed' || type === 'tool.failed') {
    return isToolPayload(value)
  }
  if (type === 'route.limit_detected') {
    return (value.kind === 'quota-exhausted' || value.kind === 'temporary-rate-limit')
      && isNonemptyText(value.message)
      && isEvidence(value.evidence)
  }
  if (type === 'adapter.diagnostic') {
    return (value.level === 'info' || value.level === 'warning' || value.level === 'error')
      && isNonemptyText(value.code, 512)
      && isNonemptyText(value.message)
      && value.terminal === false
      && isEvidence(value.evidence)
  }
  if (type === 'run.completed') {
    return isOptionalText(value.runtimeThreadId, 2_048)
      && (value.usage === undefined || isRedactedJson(value.usage))
      && isProcessEvidence(value.process)
  }
  if (type === 'run.cancelled') {
    return isOptionalText(value.runtimeThreadId, 2_048) && isProcessEvidence(value.process)
  }
  return (value.kind === 'quota-exhausted'
      || value.kind === 'temporary-rate-limit'
      || value.kind === 'authentication-failed'
      || value.kind === 'safety-blocked'
      || value.kind === 'process-failed'
      || value.kind === 'protocol-mismatch'
      || value.kind === 'unknown')
    && isNonemptyText(value.message)
    && isOptionalText(value.runtimeThreadId, 2_048)
    && (value.sessionEnded === undefined || value.sessionEnded === true)
    && (value.runtimeTerminal === 'completed' || value.runtimeTerminal === 'failed' || value.runtimeTerminal === 'missing')
    && isProcessEvidence(value.process)
}

function parsedEvent(value: unknown, metadata: MissionLedgerMetadata): NormalizedRuntimeEvent | undefined {
  if (!isObject(value)) return undefined
  if (
    !isNonemptyText(value.id, 512)
    || value.runId !== metadata.runId
    || value.missionId !== metadata.missionId
    || !Number.isSafeInteger(value.sequence)
    || (value.sequence as number) < 1
    || typeof value.type !== 'string'
    || !NORMALIZED_EVENT_TYPES.has(value.type as NormalizedRuntimeEventType)
    || !isTimestamp(value.occurredAt)
    // An event must come from the runtime the mission says it is running. A
    // Claude event inside a Codex mission means one of the two records is
    // wrong, and a ledger that accepts both cannot say which.
    || value.sourceAdapter !== metadata.runtime
    || !isObject(value.payload)
    || !isOptionalText(value.cliVersion, 256)
    || !isOptionalText(value.requestedRouteId, 256)
    || !isOptionalText(value.resolvedRouteId, 256)
    || !isOptionalText(value.runtimeThreadId, 2_048)
  ) return undefined
  if (!isEventPayload(value.type as NormalizedRuntimeEventType, value.payload)) return undefined
  return value as unknown as NormalizedRuntimeEvent
}

function parsedFailure(value: unknown): MissionHostFailure | undefined {
  if (!isObject(value)) return undefined
  const candidate = value as Partial<MissionHostFailure>
  if (
    (candidate.code !== 'runtime-start-failed' && candidate.code !== 'runtime-transport-failed')
    || typeof candidate.message !== 'string'
    || typeof candidate.occurredAt !== 'string'
  ) return undefined
  try {
    return validateFailure(candidate as MissionHostFailure)
  } catch {
    return undefined
  }
}

function phaseFor(
  events: readonly NormalizedRuntimeEvent[],
  failures: readonly MissionHostFailure[]
): RecoveredMissionPhase {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index]
    if (event?.type === 'run.completed') return 'completed'
    if (event?.type === 'run.cancelled') return 'cancelled'
    if (event?.type === 'run.failed') return 'failed'
  }
  return failures.length > 0 ? 'failed' : 'interrupted'
}

async function readLedgerFile(path: string, missionId: string): Promise<ParsedLedger> {
  const issues: MissionLedgerIssue[] = []
  let handle: FileHandle
  try {
    handle = await open(path, READ_FLAGS)
  } catch {
    return { issues: [publicIssue('read-failed', 'A mission ledger could not be read.', missionId)] }
  }

  let text: string
  try {
    const file = await handle.stat()
    if (!file.isFile()) {
      return { issues: [publicIssue('read-failed', 'A mission ledger was not a regular file.', missionId)] }
    }
    if (file.size > MAX_LEDGER_BYTES) {
      return { issues: [publicIssue('file-too-large', 'A mission ledger exceeded the local safety limit.', missionId)] }
    }
    text = await handle.readFile('utf8')
  } catch {
    return { issues: [publicIssue('read-failed', 'A mission ledger could not be read.', missionId)] }
  } finally {
    await handle.close().catch(() => undefined)
  }

  const hasTrailingNewline = text.endsWith('\n')
  const lines = text.split('\n')
  if (lines.at(-1) === '') lines.pop()
  if (!hasTrailingNewline && lines.length > 0) {
    lines.pop()
    issues.push(publicIssue(
      'truncated-tail',
      'An incomplete final ledger record was ignored after recovery.',
      missionId
    ))
  }
  if (lines.length === 0) {
    issues.push(publicIssue('invalid-record', 'The mission ledger did not contain a valid header.', missionId))
    return { issues }
  }

  let headerValue: unknown
  try {
    headerValue = JSON.parse(lines[0] ?? '') as unknown
  } catch {
    issues.push(publicIssue('invalid-record', 'The mission ledger header was invalid.', missionId))
    return { issues }
  }
  if (!isObject(headerValue)) {
    issues.push(publicIssue('invalid-record', 'The mission ledger header was invalid.', missionId))
    return { issues }
  }
  if (!isSupportedSchemaVersion(headerValue.schemaVersion)) {
    issues.push(publicIssue('unsupported-schema', 'The mission ledger uses an unsupported schema.', missionId))
    return { issues }
  }
  const schemaVersion = headerValue.schemaVersion
  const metadata = parsedMetadata(headerValue.metadata, schemaVersion)
  if (
    headerValue.recordType !== 'mission.created'
    || headerValue.ledgerSequence !== 1
    || headerValue.occurredAt !== metadata?.createdAt
    || metadata === undefined
    || metadata.missionId !== missionId
  ) {
    issues.push(publicIssue('invalid-record', 'The mission ledger header was invalid.', missionId))
    return { issues }
  }

  const events: NormalizedRuntimeEvent[] = []
  const hostFailures: MissionHostFailure[] = []
  const checkpoints: ReconciledCheckpoint[] = []
  const peerLinks: MissionPeerLink[] = []
  const editChecks: MissionEditCheck[] = []
  const approvals: MissionApproval[] = []
  let expectedSequence = 2
  let expectedEventSequence = 1
  let lastUpdatedAt = metadata.createdAt
  for (const line of lines.slice(1)) {
    if (Buffer.byteLength(line, 'utf8') > MAX_RECORD_BYTES) {
      issues.push(publicIssue('invalid-record', 'An oversized mission record and its tail were ignored.', missionId))
      break
    }
    let value: unknown
    try {
      value = JSON.parse(line) as unknown
    } catch {
      issues.push(publicIssue('invalid-record', 'An invalid mission record and its tail were ignored.', missionId))
      break
    }
    if (
      !isObject(value)
      || value.schemaVersion !== schemaVersion
      || value.ledgerSequence !== expectedSequence
      || !isTimestamp(value.occurredAt)
    ) {
      issues.push(publicIssue('invalid-record', 'A noncontiguous mission record and its tail were ignored.', missionId))
      break
    }

    if (value.recordType === 'mission.event') {
      const event = parsedEvent(value.event, metadata)
      if (
        event === undefined
        || event.sequence !== expectedEventSequence
        || value.occurredAt !== event.occurredAt
      ) {
        issues.push(publicIssue('invalid-record', 'An invalid mission event and its tail were ignored.', missionId))
        break
      }
      events.push(event)
      expectedEventSequence += 1
      lastUpdatedAt = event.occurredAt
    } else if (value.recordType === 'mission.host_failure') {
      const failure = parsedFailure(value.failure)
      if (failure === undefined || value.occurredAt !== failure.occurredAt) {
        issues.push(publicIssue('invalid-record', 'An invalid host failure and its tail were ignored.', missionId))
        break
      }
      hostFailures.push(failure)
      lastUpdatedAt = failure.occurredAt
    } else if (value.recordType === 'mission.checkpoint') {
      const checkpoint = parsedCheckpoint(value.checkpoint, metadata)
      if (
        checkpoint === undefined
        || value.occurredAt !== checkpoint.createdAt
        || checkpoint.epoch !== checkpoints.length + 1
        || checkpoint.reconciledThroughSequence > expectedEventSequence - 1
      ) {
        issues.push(publicIssue('invalid-record', 'An invalid checkpoint and its tail were ignored.', missionId))
        break
      }
      checkpoints.push(checkpoint)
      lastUpdatedAt = checkpoint.createdAt
    } else if (value.recordType === 'mission.peer') {
      const link = parsedPeerLink(value.link)
      // No writer before v5 could produce one, so a peer link in an older
      // file was written by hand -- refused for the same reason a hand-edited
      // continuation is.
      if (schemaVersion < 5 || link === undefined || value.occurredAt !== link.occurredAt) {
        issues.push(publicIssue('invalid-record', 'An invalid peer link and its tail were ignored.', missionId))
        break
      }
      peerLinks.push(link)
      lastUpdatedAt = link.occurredAt
    } else if (value.recordType === 'mission.edit_check') {
      const check = parsedEditCheck(value.check)
      if (schemaVersion < 17 || check === undefined || value.occurredAt !== check.occurredAt) {
        issues.push(publicIssue('invalid-record', 'An invalid check result and its tail were ignored.', missionId))
        break
      }
      editChecks.push(check)
      lastUpdatedAt = check.occurredAt
    } else if (value.recordType === 'mission.approval') {
      const approval = parsedApproval(value.approval, schemaVersion)
      if (schemaVersion < 20 || approval === undefined || value.occurredAt !== approval.occurredAt) {
        issues.push(publicIssue('invalid-record', 'An invalid approval record and its tail were ignored.', missionId))
        break
      }
      approvals.push(approval)
      lastUpdatedAt = approval.occurredAt
    } else {
      issues.push(publicIssue('invalid-record', 'An unknown mission record and its tail were ignored.', missionId))
      break
    }
    expectedSequence += 1
  }

  return {
    mission: {
      metadata,
      events,
      hostFailures,
      checkpoints,
      peerLinks,
      editChecks,
      approvals,
      schemaVersion,
      phase: phaseFor(events, hostFailures),
      lastUpdatedAt,
      ledgerSequence: expectedSequence - 1,
      issues
    },
    issues,
    nextSequence: expectedSequence,
    nextEventSequence: expectedEventSequence,
    byteLength: Buffer.byteLength(text, 'utf8'),
    schemaVersion
  }
}

async function syncDirectoryBestEffort(path: string): Promise<void> {
  let handle: FileHandle | undefined
  try {
    handle = await open(path, fsConstants.O_RDONLY)
    await handle.sync()
  } catch {
    // Windows does not consistently allow directory handles. The file itself is
    // still fsynced; POSIX hosts also commit the new directory entry here.
  } finally {
    await handle?.close().catch(() => undefined)
  }
}

export function createFileMissionLedger(options: FileMissionLedgerOptions): MissionLedger {
  const rootDirectory = options.rootDirectory
  if (!isAbsolute(rootDirectory) || rootDirectory.includes('\0')) {
    throw new Error('Mission ledger directory is invalid')
  }

  const now = options.now ?? (() => new Date())

  const nextSequences = new Map<string, number>()
  const nextEventSequences = new Map<string, number>()
  const nextByteOffsets = new Map<string, number>()
  const metadataByMission = new Map<string, MissionLedgerMetadata>()
  const schemaVersions = new Map<string, MissionLedgerSchemaVersion>()
  let directoryReady: Promise<void> | undefined
  let writeTail: Promise<void> = Promise.resolve()
  const removeFile = options.removeFile ?? ((path: string) => unlink(path))
  const moveFile = options.moveFile ?? ((from: string, to: string) => rename(from, to))

  const ensureDirectory = (): Promise<void> => {
    directoryReady ??= mkdir(rootDirectory, { recursive: true, mode: 0o700 }).then(() => undefined)
    return directoryReady
  }

  const invalidateCaches = (missionId: string): void => {
    nextSequences.delete(missionId)
    nextEventSequences.delete(missionId)
    nextByteOffsets.delete(missionId)
    metadataByMission.delete(missionId)
    schemaVersions.delete(missionId)
    reportDates.delete(missionId)
  }

  /**
   * What the storage report read from each file, by that file's size and
   * modified time.
   *
   * The report parsed EVERY ledger file on every Settings open to find one
   * date. MEASURED 2026-09-22 on a copy of Colin's 138-mission, 47.8 MB
   * ledger: 516-563 ms each time, nothing kept. A ledger is appended to, so a
   * file whose size and modified time are unchanged holds the record it held;
   * only the files that moved are read again -- the rule the history read has
   * used since 0.261. The date still comes from the record, never the stamp.
   */
  const reportDates = new Map<string, { readonly size: number; readonly mtimeMs: number; readonly updatedAt: number | undefined }>()

  const serialize = <T>(operation: () => Promise<T>): Promise<T> => {
    const result = writeTail.then(operation)
    writeTail = result.then(() => undefined, () => undefined)
    return result
  }

  const hydrateForAppend = async (missionId: string): Promise<{
    readonly metadata: MissionLedgerMetadata
    readonly nextSequence: number
    readonly nextEventSequence: number
    readonly byteLength: number
    readonly schemaVersion: MissionLedgerSchemaVersion
  }> => {
    const cachedMetadata = metadataByMission.get(missionId)
    const cachedSequence = nextSequences.get(missionId)
    const cachedEventSequence = nextEventSequences.get(missionId)
    const cachedByteLength = nextByteOffsets.get(missionId)
    const cachedSchemaVersion = schemaVersions.get(missionId)
    if (
      cachedMetadata !== undefined
      && cachedSequence !== undefined
      && cachedEventSequence !== undefined
      && cachedByteLength !== undefined
      && cachedSchemaVersion !== undefined
    ) {
      return {
        metadata: cachedMetadata,
        nextSequence: cachedSequence,
        nextEventSequence: cachedEventSequence,
        byteLength: cachedByteLength,
        schemaVersion: cachedSchemaVersion
      }
    }
    await ensureDirectory()
    const parsed = await readLedgerFile(missionPath(rootDirectory, missionId), missionId)
    if (
      parsed.mission === undefined
      || parsed.nextSequence === undefined
      || parsed.nextEventSequence === undefined
      || parsed.byteLength === undefined
      || parsed.schemaVersion === undefined
      || parsed.issues.length > 0
    ) {
      throw new Error('Mission ledger is unavailable')
    }
    metadataByMission.set(missionId, parsed.mission.metadata)
    nextSequences.set(missionId, parsed.nextSequence)
    nextEventSequences.set(missionId, parsed.nextEventSequence)
    nextByteOffsets.set(missionId, parsed.byteLength)
    schemaVersions.set(missionId, parsed.schemaVersion)
    return {
      metadata: parsed.mission.metadata,
      nextSequence: parsed.nextSequence,
      nextEventSequence: parsed.nextEventSequence,
      schemaVersion: parsed.schemaVersion,
      byteLength: parsed.byteLength
    }
  }

  /**
   * Write these records, in as many appends as it takes.
   *
   * `MAX_EVENTS_PER_APPEND` and `MAX_APPEND_BYTES` bound ONE WRITE -- they
   * are about how much this file is willing to put in a single append, not
   * about how much a caller is allowed to record. They were enforced as a
   * refusal, and on 2026-09-21 Colin lost a finished Cursor run to it: a
   * long answer arrived as one burst, the normalizer turned it into more
   * than a hundred events, and the mission was STOPPED with "Too many
   * mission events in one append" -- the analysis already on screen, and the
   * ledger refusing to keep it.
   *
   * A batch too big for one write is not a caller error; it is a write to
   * split. Every record is still validated before any of it is written, and
   * the sequences were assigned in one pass, so the batches land in order
   * and read back as one run. A failure part-way leaves the earlier batches
   * on disk, which is what an append-only ledger promises anyway -- "the
   * ledger only ever appends, so what reached the disk is intact" is what
   * the failure card already tells the person.
   */
  const appendInBatches = async (missionId: string, records: readonly LedgerRecord[]): Promise<void> => {
    let batch: LedgerRecord[] = []
    let bytes = 0
    for (const record of records) {
      // Each record is under `MAX_RECORD_BYTES` (checked by `recordLine`),
      // which is a quarter of the append bound, so a single record always
      // fits in a batch of its own and this loop always makes progress.
      const size = Buffer.byteLength(recordLine(record), 'utf8')
      if (batch.length > 0 && (batch.length >= MAX_EVENTS_PER_APPEND || bytes + size > MAX_APPEND_BYTES)) {
        await appendRecords(missionId, batch)
        batch = []
        bytes = 0
      }
      batch.push(record)
      bytes += size
    }
    if (batch.length > 0) await appendRecords(missionId, batch)
  }

  const appendRecords = async (missionId: string, records: readonly LedgerRecord[]): Promise<void> => {
    const lines = records.map(recordLine).join('')
    const appendBytes = Buffer.byteLength(lines, 'utf8')
    if (appendBytes > MAX_APPEND_BYTES) {
      throw new Error('Mission ledger append exceeds its safety limit')
    }
    const expectedBytes = nextByteOffsets.get(missionId)
    if (expectedBytes === undefined) throw new Error('Mission ledger offset is unavailable')
    try {
      const handle = await openForAppend(missionPath(rootDirectory, missionId))
      try {
        const file = await handle.stat()
        if (!file.isFile() || file.size !== expectedBytes) {
          throw new Error('Mission ledger changed outside the active writer')
        }
        if (file.size + appendBytes > MAX_LEDGER_BYTES) {
          throw new Error('Mission ledger exceeds its safety limit')
        }
        await handle.writeFile(lines, 'utf8')
        await handle.sync()
        nextByteOffsets.set(missionId, file.size + appendBytes)
      } finally {
        await handle.close()
      }
    } catch (error) {
      // The on-disk state is now uncertain (a write may have landed without its
      // sync, or the file changed externally). Drop the cached offsets so the
      // next append re-hydrates from the file instead of wedging the mission.
      invalidateCaches(missionId)
      throw error
    }
  }

  /**
   * Open the ledger for appending, waiting out a transient lock.
   *
   * On Windows a file can refuse to open for a few milliseconds because
   * something else is holding it -- a virus scanner reading it as it grows,
   * an indexer, a backup agent. Nothing is wrong with the ledger; the moment
   * is wrong.
   *
   * Before this there was no retry, so one such moment ended the run. Colin,
   * 2026-09-15, on a live Cursor mission: "Stopped -- the mission ledger
   * could not be written." His file was perfectly intact afterwards -- 449
   * records, every one parsing, sequence contiguous 1 to 449, 454KB against
   * a 64MB cap -- which is what says the failure was the open and not the
   * contents.
   *
   * ONLY THE OPEN IS RETRIED, and that is the whole safety argument. Nothing
   * has been written at this point, so trying again cannot tear a record. A
   * failure during `writeFile` or `sync` still throws immediately, because
   * there the on-disk state is genuinely uncertain and the app's answer --
   * stop rather than continue without a durable record -- is the right one.
   *
   * And only for codes that mean "busy". `ENOSPC` is a full disk and
   * `EROFS` a read-only one; retrying those is waiting for a fact to change
   * that will not, and it would turn a clear failure into a slow one.
   */
  async function openForAppend(path: string): Promise<FileHandle> {
    const BUSY = new Set(['EBUSY', 'EPERM', 'EACCES'])
    const WAITS = [25, 50, 100]
    for (let attempt = 0; ; attempt += 1) {
      try {
        return await open(path, APPEND_FLAGS, 0o600)
      } catch (error) {
        const code = (error as NodeJS.ErrnoException).code ?? ''
        const wait = WAITS[attempt]
        if (!BUSY.has(code) || wait === undefined) throw error
        await new Promise((resolve) => setTimeout(resolve, wait))
      }
    }
  }

  /**
   * Run `work` over `items` a few at a time.
   *
   * These loops opened every ledger at once. With a few hundred missions that
   * exhausts the process's file descriptors, and an `EMFILE` reads exactly
   * like a corrupt ledger -- which, for a prune, is the difference between
   * protecting a conversation and deleting its opening turns.
   */
  async function mapLimited<TIn, TOut>(
    items: readonly TIn[],
    limit: number,
    work: (item: TIn) => Promise<TOut>
  ): Promise<TOut[]> {
    const results: TOut[] = new Array<TOut>(items.length)
    let next = 0
    const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
      for (let index = next++; index < items.length; index = next++) {
        results[index] = await work(items[index]!)
      }
    })
    await Promise.all(workers)
    return results
  }

  /**
   * The mission a file says it continues, read from its first line alone.
   *
   * A full read can refuse a file for reasons that say nothing about its first
   * line: a corrupt record near the end, a file over the size cap, a schema
   * from a newer build. The link still matters -- it is what stops an old
   * parent being deleted out from under a conversation still in use -- so it
   * is read leniently here, and used ONLY to protect, never to delete.
   */
  async function continuationHint(missionId: string): Promise<
    { readonly ok: true; readonly continuesFrom?: string } | { readonly ok: false }
  > {
    let handle
    try {
      handle = await open(missionPath(rootDirectory, missionId), 'r')
    } catch (error) {
      // A file that is not there cannot protect anything and is not a failure.
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return { ok: true }
      return { ok: false }
    }
    try {
      const head = Buffer.alloc(64 * 1024)
      const { bytesRead } = await handle.read(head, 0, head.length, 0)
      const text = head.subarray(0, bytesRead).toString('utf8')
      const firstLine = text.split('\n', 1)[0] ?? ''
      const value: unknown = JSON.parse(firstLine)
      if (!isObject(value) || !isObject(value.metadata)) return { ok: true }
      const link = (value.metadata as { readonly continuesFrom?: unknown }).continuesFrom
      const parent = isObject(link) ? link.missionId : undefined
      return typeof parent === 'string' && SAFE_ID.test(parent)
        ? { ok: true, continuesFrom: parent }
        : { ok: true }
    } catch {
      // Unparseable is not unreadable: the file was opened and read. It simply
      // declares no link, which is the same as having none.
      return { ok: true }
    } finally {
      await handle.close().catch(() => undefined)
    }
  }

  /**
   * Remove one mission file. No cache is cleared here, on purpose. An append
   * afterwards either re-creates the mission (which rewrites every cached
   * counter) or opens the file without O_CREAT (which fails on the missing
   * file and clears the cache in its own error path). A clearing line was
   * here once; the mutation control proved no test could tell whether it ran,
   * and a line no test can see is a line nobody maintains.
   */
  /**
   * Move a mission's record to the trash. Returns false when there was none.
   *
   * MEASURED 2026-09-17: eighteen of Colin's missions went in four seconds
   * through the app's own confirm, and the files were unlinked -- no Recycle
   * Bin entry, no shadow copy, nothing to undo. A day of work was recovered
   * only because those runs happened to be on Cursor, which keeps its own
   * transcripts. A product whose claim is a durable local record cannot rely
   * on another program's copy for that.
   *
   * So the file is RENAMED rather than removed. A rename inside one directory
   * is atomic and copies no bytes, so this costs nothing on a large ledger,
   * and the record is byte-identical if it comes back.
   *
   * The mtime is restamped to now, deliberately: that is what `listTrashed`
   * reports as the moment it was deleted, and it means the trash needs no
   * index of its own to keep in step with the files in it. Nothing reads a
   * trashed mission's last-updated time -- `storageReport` only looks at
   * files in the root -- so no information anyone uses is lost.
   */
  async function removeMissionFile(missionId: string): Promise<boolean> {
    requireSafeId(missionId, 'missionId')
    const from = missionPath(rootDirectory, missionId)
    const to = trashPath(rootDirectory, missionId)
    try {
      await mkdir(join(rootDirectory, MISSION_TRASH_DIR), { recursive: true, mode: 0o700 })
      // A mission deleted, restored and deleted again would land on its own
      // older copy. Rename replaces on both platforms this ships to, but
      // saying so here is cheaper than depending on it.
      await removeFile(to).catch(() => undefined)
      await moveFile(from, to)
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false
      throw error
    }
    const now = new Date()
    await utimes(to, now, now).catch(() => undefined)
    await syncDirectoryBestEffort(rootDirectory)
    return true
  }

  /** Every mission in the trash, newest deletion first. */
  async function listTrashedFiles(): Promise<readonly TrashedMission[]> {
    let entries
    try {
      entries = await readdir(join(rootDirectory, MISSION_TRASH_DIR), { withFileTypes: true })
    } catch {
      return []
    }
    const found: TrashedMission[] = []
    for (const entry of entries) {
      if (!entry.isFile() || !entry.name.endsWith('.jsonl')) continue
      const missionId = entry.name.slice(0, -'.jsonl'.length)
      const path = join(rootDirectory, MISSION_TRASH_DIR, entry.name)
      let deletedAt: string
      let bytes = 0
      try {
        const info = await stat(path)
        deletedAt = info.mtime.toISOString()
        bytes = info.size
      } catch {
        continue
      }
      found.push({ missionId, deletedAt, bytes, ...(await headerOf(path)) })
    }
    return found.sort((left, right) => (left.deletedAt < right.deletedAt ? 1 : -1))
  }

  /**
   * The first record of a file, for a trash listing that can name what it is
   * holding. Reads a chunk rather than the whole mission: a listing must not
   * cost what reading the history costs.
   */
  async function headerOf(path: string): Promise<{ prompt?: string; createdAt?: string }> {
    let handle: FileHandle | undefined
    try {
      handle = await open(path, 'r')
      const chunk = Buffer.alloc(16_384)
      const { bytesRead } = await handle.read(chunk, 0, chunk.length, 0)
      const text = chunk.subarray(0, bytesRead).toString('utf8')
      const line = text.split(String.fromCharCode(10))[0] ?? ''
      const parsed = JSON.parse(line) as { metadata?: { prompt?: unknown; createdAt?: unknown } }
      const prompt = parsed.metadata?.prompt
      const createdAt = parsed.metadata?.createdAt
      return {
        ...(typeof prompt === 'string' ? { prompt: prompt.slice(0, 200) } : {}),
        ...(typeof createdAt === 'string' ? { createdAt } : {})
      }
    } catch {
      // A trashed mission that cannot be read is still listed, by id alone.
      return {}
    } finally {
      await handle?.close().catch(() => undefined)
    }
  }

  return {
    createMission(metadata: MissionLedgerMetadata): Promise<void> {
      return serialize(async () => {
        validateMetadata(metadata)
        await ensureDirectory()
        const record: CreatedRecord = {
          schemaVersion: MISSION_LEDGER_SCHEMA_VERSION,
          recordType: 'mission.created',
          ledgerSequence: 1,
          occurredAt: metadata.createdAt,
          metadata
        }
        const line = recordLine(record)
        const handle = await open(missionPath(rootDirectory, metadata.missionId), 'wx', 0o600)
        try {
          await handle.writeFile(line, 'utf8')
          await handle.sync()
        } finally {
          await handle.close()
        }
        await syncDirectoryBestEffort(rootDirectory)
        metadataByMission.set(metadata.missionId, metadata)
        nextSequences.set(metadata.missionId, 2)
        nextEventSequences.set(metadata.missionId, 1)
        schemaVersions.set(metadata.missionId, MISSION_LEDGER_SCHEMA_VERSION)
        nextByteOffsets.set(metadata.missionId, Buffer.byteLength(line, 'utf8'))
      })
    },

    appendEvents(
      missionId: string,
      events: readonly NormalizedRuntimeEvent[]
    ): Promise<void> {
      return serialize(async () => {
        requireSafeId(missionId, 'missionId')
        if (events.length === 0) return
        const hydrated = await hydrateForAppend(missionId)
        let sequence = hydrated.nextSequence
        let eventSequence = hydrated.nextEventSequence
        // Sizes are fitted here, once, for every caller (fit.ts): an event
        // that is merely long is kept shortened, never refused.
        const records: EventRecord[] = fitEventsToLedger(events).map((event) => {
          if (event.missionId !== missionId || event.runId !== hydrated.metadata.runId) {
            throw new Error('Mission event correlation is invalid')
          }
          if (event.sequence !== eventSequence) {
            throw new Error('Mission event sequence is invalid')
          }
          // Writer strictness is defined BY the reader, not agreed with it. An
          // event the reader would refuse is unwritable: recovery stops at the
          // first record it cannot parse, so accepting one here would silently
          // discard every later record of an otherwise intact mission.
          if (parsedEvent(event, hydrated.metadata) === undefined) {
            throw new Error('Mission event is not readable by the ledger reader')
          }
          const record: EventRecord = {
            // The file's version, not the writer's. A v1 mission that starts
            // receiving v2 records becomes a file no reader can walk end to end.
            schemaVersion: hydrated.schemaVersion,
            recordType: 'mission.event',
            ledgerSequence: sequence,
            occurredAt: requireTimestamp(event.occurredAt, 'Event timestamp'),
            event
          }
          sequence += 1
          eventSequence += 1
          return record
        })
        await appendInBatches(missionId, records)
        nextSequences.set(missionId, sequence)
        nextEventSequences.set(missionId, eventSequence)
      })
    },

    appendHostFailure(missionId: string, failure: MissionHostFailure): Promise<void> {
      return serialize(async () => {
        requireSafeId(missionId, 'missionId')
        validateFailure(failure)
        const hydrated = await hydrateForAppend(missionId)
        const record: HostFailureRecord = {
          schemaVersion: hydrated.schemaVersion,
          recordType: 'mission.host_failure',
          ledgerSequence: hydrated.nextSequence,
          occurredAt: failure.occurredAt,
          failure
        }
        await appendRecords(missionId, [record])
        nextSequences.set(missionId, hydrated.nextSequence + 1)
      })
    },

    createCheckpoint(missionId: string, reason: CheckpointReason): Promise<ReconciledCheckpoint> {
      return serialize(async () => {
        requireSafeId(missionId, 'missionId')
        // Read the FILE first, before hydrating anything. Reconciliation runs
        // against what survived an fsync, never against cached counters -- the
        // cached view is exactly what an interrupted process leaves behind.
        // Reading first also matters for a damaged ledger: `hydrateForAppend`
        // fails closed on integrity issues, so hydrating up front would turn
        // the most important verdict this function can return into an opaque
        // throw, and the caller needs the reason, not just the refusal.
        const parsed = await readLedgerFile(missionPath(rootDirectory, missionId), missionId)
        const mission = parsed.mission
        if (mission === undefined) throw new Error('Mission ledger could not be reconciled')
        const checkpoint = reconcileMission(
          { metadata: mission.metadata, events: mission.events, issues: mission.issues },
          {
            reason,
            epoch: mission.checkpoints.length + 1,
            createdAt: now().toISOString()
          }
        )
        // Same rule as events: the reader defines what is writable.
        if (parsedCheckpoint(checkpoint, mission.metadata) === undefined) {
          throw new Error('Checkpoint is not readable by the ledger reader')
        }
        // An already-broken ledger cannot be checkpointed durably. Recovery
        // stops at the first record it cannot parse, so anything appended past
        // that break is written, fsynced, and permanently unreachable -- the
        // exact shape of the defect this ledger was hardened against. The
        // verdict still goes back to the caller, and `unsafe` is the strongest
        // answer it can give, so nothing is lost by declining to write it.
        if (checkpoint.resumeSafety === 'unsafe') return checkpoint
        const hydrated = await hydrateForAppend(missionId)
        const record: CheckpointRecord = {
          schemaVersion: hydrated.schemaVersion,
          recordType: 'mission.checkpoint',
          ledgerSequence: hydrated.nextSequence,
          occurredAt: checkpoint.createdAt,
          checkpoint
        }
        await appendRecords(missionId, [record])
        nextSequences.set(missionId, hydrated.nextSequence + 1)
        return checkpoint
      })
    },

    appendPeerLinks(missionId: string, links: readonly MissionPeerLink[]): Promise<void> {
      return serialize(async () => {
        requireSafeId(missionId, 'missionId')
        if (links.length === 0) return
        if (links.length > MAX_EVENTS_PER_APPEND) throw new Error('Too many peer links in one append')
        for (const link of links) validatePeerLink(link)
        const hydrated = await hydrateForAppend(missionId)
        if (hydrated.schemaVersion < 5) {
          // The file's version is fixed by its header. A pre-v5 mission cannot
          // take a record its own readers would stop at.
          throw new Error('Mission ledger version cannot hold peer links')
        }
        let sequence = hydrated.nextSequence
        const records: PeerRecord[] = links.map((link) => {
          const record: PeerRecord = {
            schemaVersion: hydrated.schemaVersion,
            recordType: 'mission.peer',
            ledgerSequence: sequence,
            occurredAt: link.occurredAt,
            link
          }
          sequence += 1
          return record
        })
        await appendRecords(missionId, records)
        nextSequences.set(missionId, sequence)
      })
    },

    appendEditCheck(missionId: string, check: MissionEditCheck): Promise<void> {
      return serialize(async () => {
        requireSafeId(missionId, 'missionId')
        validateEditCheck(check)
        const hydrated = await hydrateForAppend(missionId)
        if (hydrated.schemaVersion < 17) {
          // Same rule as a peer link on a pre-v5 file: an older mission cannot
          // take a record its own readers would stop at. Its card is simply
          // not kept.
          throw new Error('Mission ledger version cannot hold check results')
        }
        const sequence = hydrated.nextSequence
        const record: EditCheckRecord = {
          schemaVersion: hydrated.schemaVersion,
          recordType: 'mission.edit_check',
          ledgerSequence: sequence,
          occurredAt: check.occurredAt,
          check
        }
        await appendRecords(missionId, [record])
        nextSequences.set(missionId, sequence + 1)
      })
    },

    appendApproval(missionId: string, approval: MissionApproval): Promise<void> {
      return serialize(async () => {
        requireSafeId(missionId, 'missionId')
        validateApproval(approval)
        const hydrated = await hydrateForAppend(missionId)
        if (hydrated.schemaVersion < 20) {
          // Same rule as a check result on a pre-v17 file: an older mission
          // cannot take a record its own readers would stop at.
          throw new Error('Mission ledger version cannot hold approvals')
        }
        if (approval.by === 'earlier-always' && hydrated.schemaVersion < 21) {
          throw new Error('Mission ledger version cannot hold an answer by an earlier Always')
        }
        const sequence = hydrated.nextSequence
        const record: ApprovalRecord = {
          schemaVersion: hydrated.schemaVersion,
          recordType: 'mission.approval',
          ledgerSequence: sequence,
          occurredAt: approval.occurredAt,
          approval
        }
        await appendRecords(missionId, [record])
        nextSequences.set(missionId, sequence + 1)
      })
    },

    deleteMission(missionId: string): Promise<boolean> {
      return serialize(() => removeMissionFile(missionId))
    },

    listTrashedMissions(): Promise<readonly TrashedMission[]> {
      return serialize(() => listTrashedFiles())
    },

    restoreMission(missionId: string): Promise<boolean> {
      return serialize(async () => {
        requireSafeId(missionId, 'missionId')
        const from = trashPath(rootDirectory, missionId)
        const to = missionPath(rootDirectory, missionId)
        // Never over a record that exists. A mission id is unique, so this
        // means the same id ran again; the live one is the truth.
        try {
          await stat(to)
          return false
        } catch {
          // Nothing there, which is the ordinary case.
        }
        try {
          await moveFile(from, to)
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false
          throw error
        }
        await syncDirectoryBestEffort(rootDirectory)
        return true
      })
    },

    emptyTrash(): Promise<number> {
      return serialize(async () => {
        const held = await listTrashedFiles()
        let gone = 0
        for (const entry of held) {
          try {
            await removeFile(trashPath(rootDirectory, entry.missionId))
            gone += 1
          } catch {
            // One that will not go does not stop the rest.
          }
        }
        await syncDirectoryBestEffort(rootDirectory)
        return gone
      })
    },

    async storageReport(): Promise<MissionStorageReport> {
      await writeTail
      await ensureDirectory()
      let entries
      try {
        entries = await readdir(rootDirectory, { withFileTypes: true })
      } catch {
        return { missionCount: 0, byteTotal: 0, unreadableCount: 0 }
      }
      const ids = entries
        .filter((entry) => entry.isFile() && entry.name.endsWith('.jsonl'))
        .map((entry) => entry.name.slice(0, -'.jsonl'.length))
        .filter((missionId) => SAFE_ID.test(missionId))
      let byteTotal = 0
      let oldest: number | undefined
      let counted = 0
      let unreadableCount = 0
      const present = new Set<string>()
      await mapLimited(ids, READ_CONCURRENCY, async (missionId) => {
        let file
        try {
          file = await stat(missionPath(rootDirectory, missionId))
          byteTotal += file.size
          counted += 1
          present.add(missionId)
        } catch {
          // A file that vanished between listing and stat is not history.
          return
        }
        // The DATE comes from the record, not from the file's timestamp. A
        // prune judges by `lastUpdatedAt`, so a report dated by mtime could
        // tell someone their history reaches back further -- or less far --
        // than the thing they are about to press would act on. Copying a
        // ledger directory is enough to make the two disagree.
        //
        // The file's size and modified time only decide whether the record
        // needs reading AGAIN: see `reportDates`.
        const held = reportDates.get(missionId)
        let updatedAt: number | undefined
        if (held !== undefined && held.size === file.size && held.mtimeMs === file.mtimeMs) {
          updatedAt = held.updatedAt
        } else {
          const parsed = await readLedgerFile(missionPath(rootDirectory, missionId), missionId)
          const read = parsed.mission === undefined ? undefined : Date.parse(parsed.mission.lastUpdatedAt)
          updatedAt = read !== undefined && Number.isFinite(read) ? read : undefined
          reportDates.set(missionId, { size: file.size, mtimeMs: file.mtimeMs, updatedAt })
        }
        if (updatedAt === undefined) {
          // Counted in the total above, so its absence from the date has to be
          // stated rather than left to look like a mission with no history.
          unreadableCount += 1
          return
        }
        if (oldest === undefined || updatedAt < oldest) oldest = updatedAt
      })
      for (const missionId of [...reportDates.keys()]) {
        if (!present.has(missionId)) reportDates.delete(missionId)
      }
      return {
        missionCount: counted,
        byteTotal,
        unreadableCount,
        ...(oldest === undefined ? {} : { oldestUpdatedAt: new Date(oldest).toISOString() })
      }
    },

    async pruneMissions(options: MissionPruneOptions): Promise<MissionPruneResult> {
      const cutoff = Date.parse(requireTimestamp(options.before, 'before'))
      const running = new Set(options.protectMissionIds ?? [])
      // A confirmation names the exact missions its preview showed. Deleting
      // is then bounded by that list, so whatever changed between the two --
      // the clock, a new reply, a mission deleted by hand -- the result can
      // only ever be NARROWER than what the person agreed to.
      const only = options.only === undefined ? undefined : new Set(options.only)
      await ensureDirectory()
      // The whole prune runs as one queued operation. Reading the directory
      // outside the queue let a mission be created between the read and the
      // unlink, and a reply written in that gap left its parent unprotected.
      return serialize(async () => {
        let entries
        try {
          entries = await readdir(rootDirectory, { withFileTypes: true })
        } catch {
          return { deleted: [], failed: [], unreadable: [], keptForContinuity: [], keptAsRunning: [] }
        }
        const named = entries
          .filter((entry) => entry.name.endsWith('.jsonl'))
          .map((entry) => ({ missionId: entry.name.slice(0, -'.jsonl'.length), isFile: entry.isFile() }))
          .filter((entry) => SAFE_ID.test(entry.missionId))
        const ids = named.filter((entry) => entry.isFile).map((entry) => entry.missionId)
        // A mission's name occupied by something that is not a regular file
        // cannot be read, and its links cannot be known. Listing skips such an
        // entry silently; a prune must not, because the thing it cannot read
        // may be the newest turn of a conversation it is about to cut off.
        const occupied = named.filter((entry) => !entry.isFile).map((entry) => entry.missionId)

        // Read every header once: which missions are old enough, and which
        // mission each one continues from.
        const held = await mapLimited(ids, READ_CONCURRENCY, async (missionId) => {
          const parsed = await readLedgerFile(missionPath(rootDirectory, missionId), missionId)
          const mission = parsed.mission
          // A file the full reader refuses can still declare who it continues,
          // and that link is what protects an old parent. Read leniently, and
          // separately note a file that could not be read AT ALL.
          const hint = mission === undefined ? await continuationHint(missionId) : { ok: true as const }
          return {
            missionId,
            readable: mission !== undefined,
            openable: hint.ok,
            lastUpdatedAt: mission === undefined ? undefined : Date.parse(mission.lastUpdatedAt),
            continuesFrom: mission?.metadata.continuesFrom?.missionId
              ?? (hint.ok ? hint.continuesFrom : undefined)
          }
        })

        // If a ledger cannot be opened, its links are unknowable, and one of
        // them may be the newest turn of a conversation whose opening mission
        // is a candidate here. Nothing is deleted until it can be read.
        const unreadable = [...occupied, ...held.filter((entry) => !entry.openable).map((entry) => entry.missionId)]
        if (unreadable.length > 0) {
          return { deleted: [], failed: [], unreadable, keptForContinuity: [], keptAsRunning: [] }
        }

        const parents = new Map<string, string>()
        for (const entry of held) {
          if (entry.continuesFrom !== undefined) parents.set(entry.missionId, entry.continuesFrom)
        }

        const keptAsRunning: string[] = []
        const candidates = new Set<string>()
        const survivors: string[] = []
        for (const entry of held) {
          const old = entry.readable
            && entry.lastUpdatedAt !== undefined
            && Number.isFinite(entry.lastUpdatedAt)
            && entry.lastUpdatedAt < cutoff
          if (!old || (only !== undefined && !only.has(entry.missionId))) {
            survivors.push(entry.missionId)
            continue
          }
          if (running.has(entry.missionId)) {
            keptAsRunning.push(entry.missionId)
            survivors.push(entry.missionId)
            continue
          }
          candidates.add(entry.missionId)
        }

        // Walk back from everything that survives. A conversation is a chain
        // of missions, so the whole chain behind a survivor is protected, not
        // just its immediate parent -- three turns back is still that
        // conversation.
        const keptForContinuity = new Set<string>()
        for (const survivor of survivors) {
          let cursor = parents.get(survivor)
          const seen = new Set<string>([survivor])
          while (cursor !== undefined && !seen.has(cursor)) {
            seen.add(cursor)
            if (candidates.has(cursor)) {
              candidates.delete(cursor)
              keptForContinuity.add(cursor)
            }
            cursor = parents.get(cursor)
          }
        }

        const deleted: string[] = []
        const failed: string[] = []
        for (const missionId of candidates) {
          if (options.dryRun === true) {
            deleted.push(missionId)
            continue
          }
          // Per file. A lock on one must not throw away the true record of
          // the ones already gone, which would report "nothing happened"
          // about something irreversible.
          try {
            if (await removeMissionFile(missionId)) deleted.push(missionId)
          } catch {
            failed.push(missionId)
          }
        }
        return {
          deleted,
          failed,
          unreadable: [],
          keptForContinuity: [...keptForContinuity],
          keptAsRunning
        }
      })
    },

    async getMission(missionId: string): Promise<RecoveredMission | undefined> {
      requireSafeId(missionId, 'missionId')
      await writeTail
      await ensureDirectory()
      const parsed = await readLedgerFile(missionPath(rootDirectory, missionId), missionId)
      return parsed.mission
    },

    async missionFiles(): Promise<MissionFileListing> {
      await writeTail
      await ensureDirectory()
      let entries
      try {
        entries = await readdir(rootDirectory, { withFileTypes: true })
      } catch {
        return { files: [], issues: [publicIssue('read-failed', 'Local mission history could not be read.')] }
      }
      const ids = entries
        .filter((entry) => entry.isFile() && entry.name.endsWith('.jsonl'))
        .map((entry) => entry.name.slice(0, -'.jsonl'.length))
        .filter((missionId) => SAFE_ID.test(missionId))
      const stamped = await mapLimited(ids, READ_CONCURRENCY, async (missionId) => {
        try {
          const file = await stat(missionPath(rootDirectory, missionId))
          return { missionId, modifiedAt: file.mtimeMs, size: file.size }
        } catch {
          // Sorted last rather than dropped, as `listMissions` does, so a
          // file that cannot be read can still surface its own issue.
          return { missionId, modifiedAt: 0, size: -1 }
        }
      })
      stamped.sort((left, right) => right.modifiedAt - left.modifiedAt)
      const over = stamped.length > MAX_MISSION_FILES
      return {
        files: over ? stamped.slice(0, MAX_MISSION_FILES) : stamped,
        totalFiles: stamped.length,
        issues: over
          ? [publicIssue('file-limit-exceeded', `Only the ${MAX_MISSION_FILES} most recently updated local mission ledgers were inspected.`)]
          : []
      }
    },

    async readMission(missionId: string): Promise<MissionFileRead> {
      requireSafeId(missionId, 'missionId')
      await writeTail
      const parsed = await readLedgerFile(missionPath(rootDirectory, missionId), missionId)
      return parsed.mission === undefined ? { issues: parsed.issues } : { mission: parsed.mission, issues: parsed.issues }
    },

    async listMissions(options?: MissionLedgerListOptions): Promise<MissionLedgerSnapshot> {
      const limit = options?.limit === undefined
        ? DEFAULT_MISSION_LIST_LIMIT
        : Math.min(Math.max(Math.trunc(options.limit), 1), MAX_MISSION_FILES)
      await writeTail
      await ensureDirectory()
      let entries
      try {
        entries = await readdir(rootDirectory, { withFileTypes: true })
      } catch {
        return {
          missions: [],
          issues: [publicIssue('read-failed', 'Local mission history could not be read.')],
          // The directory itself would not open, so no file was read and none
          // can be said to be unreadable. The caller distinguishes this from a
          // damaged file by the read-failed issue, not by a count.
          unreadableCount: 0
        }
      }
      const allIds = entries
        .filter((entry) => entry.isFile() && entry.name.endsWith('.jsonl'))
        .map((entry) => entry.name.slice(0, -'.jsonl'.length))
        .filter((missionId) => SAFE_ID.test(missionId))
      const issues: MissionLedgerIssue[] = []

      // Choose WHICH files to read by recency, not by filename. Mission ids are
      // random, so sorting by name and taking the first N gives an arbitrary
      // subset -- and past the cap the user would be shown whichever missions
      // happened to sort early rather than the ones they last worked on. The
      // sort further down orders what was read; this decides what gets read at
      // all, which is the part that was wrong.
      let candidateIds = allIds
      if (allIds.length > MAX_MISSION_FILES) {
        const stamped = await mapLimited(allIds, READ_CONCURRENCY, async (missionId) => {
          try {
            const file = await stat(missionPath(rootDirectory, missionId))
            return { missionId, modifiedAt: file.mtimeMs }
          } catch {
            // Unreadable now is likely unreadable in a moment; sort it last
            // rather than dropping it, so it can still surface its own issue.
            return { missionId, modifiedAt: 0 }
          }
        })
        stamped.sort((left, right) => right.modifiedAt - left.modifiedAt)
        candidateIds = stamped.slice(0, MAX_MISSION_FILES).map((entry) => entry.missionId)
        issues.push(publicIssue(
          'file-limit-exceeded',
          `Only the ${MAX_MISSION_FILES} most recently updated local mission ledgers were inspected.`
        ))
      }

      const parsed = await mapLimited(candidateIds, READ_CONCURRENCY, async (missionId) =>
        readLedgerFile(missionPath(rootDirectory, missionId), missionId))
      const missions = parsed
        .flatMap((result) => result.mission === undefined ? [] : [result.mission])
        .sort((left, right) => Date.parse(right.lastUpdatedAt) - Date.parse(left.lastUpdatedAt))
        .slice(0, limit)
      for (const result of parsed) issues.push(...result.issues)
      /*
       * Counted from `parsed`, BEFORE the slice above.
       *
       * `missions` is a page; `issues` is not. Deriving this by asking which
       * issue ids are missing from `missions` therefore counts every recovered
       * mission that fell off the page as a file that could not be read --
       * measured by Astra, 2026-09-09, as "20 local · 1 file could not be
       * read" with nothing wrong. A parse result either produced a mission or
       * it did not, and that is known right here and nowhere else.
       */
      const unreadableCount = parsed.filter((result) => result.mission === undefined).length
      return { missions, issues, unreadableCount }
    },

    flush(): Promise<void> {
      return writeTail
    }
  }
}

export {
  createFileWorkroom,
  isWorkroomName,
  isObservedPath,
  isWorkroomText,
  MAX_OBSERVED_PATHS,
  MAX_WORKROOM_MESSAGE_LENGTH,
  MAX_WORKROOM_NAME_LENGTH,
  WORKROOM_SCHEMA_VERSION
} from './workroom.js'
export type {
  FileWorkroomOptions,
  Workroom,
  WorkroomDelivery,
  WorkroomIssue,
  WorkroomIssueCode,
  WorkroomMessage,
  WorkroomParty,
  WorkroomSender,
  WorkroomSnapshot,
  WorkroomUnread
} from './workroom.js'
export {
  fitEventToLedger,
  fitEventsToLedger,
  fitJson,
  fitText,
  LEDGER_NAME_LIMIT,
  LEDGER_TEXT_LIMIT
} from './fit.js'
export {
  CHECKPOINT_SCHEMA_VERSION,
  MAX_CHECKPOINT_SUMMARY_LENGTH,
  MAX_SETTLED_NAMES,
  MAX_UNSETTLED_ACTIONS,
  parsedCheckpoint,
  reconcileMission
} from './checkpoint.js'
export type {
  CheckpointReason,
  CheckpointResumeSafety,
  ReconciledCheckpoint,
  ReconcileMissionInput,
  ReconcileMissionOptions,
  UnsettledAction
} from './checkpoint.js'
