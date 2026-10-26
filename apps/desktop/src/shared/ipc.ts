import type { AccountAction, AgentAccountsState } from './agent-accounts.js'
import type { AppearanceState, ThemeChoice } from './appearance.js'
import type { ReverseChange } from './reverse-diff.js'
import type { AwaySummaryCounts } from './away.js'
import type { RemoteControlState } from './claude-remote-control.js'
import type { CommitResult, CommitThen, FolderChanges } from './folder-commit.js'
import type { ListedIssue, StartedIssue } from './github-issue.js'
import type { BackgroundStartRequest, BackgroundStartResponse, PublicBackgroundRun } from './background.js'
import type { MissionRuntimeId, NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import type { QueuedMessagesResponse, SavedQueuedMessage } from './queued-messages.js'
import type { DiagramResponse } from './diagram.js'

import type { AvatarSpec, PetSource } from './avatar.js'
import type { PetRows } from './pets.js'

import type { RoutineSchedule } from './routine-schedule.js'
export type { RoutineSchedule } from './routine-schedule.js'
import type { RoutineInput } from './routine-inputs.js'
export type { RoutineInput, RoutineInputKind } from './routine-inputs.js'
import type { MemoryScope } from './memory.js'
export type { MemoryScope } from './memory.js'
import type { Spend } from './spend.js'
import type { UsageRange, UsageSummary } from './usage.js'
import type { ConnectorAddRequest, ConnectorAddResponse, ConnectorAgent } from './connector-add.js'
import type { GithubAccount, GithubCliVersions, GithubInstallResult, GithubSignInCode, GithubSignInResult } from './github-account.js'
import type { FolderPullRequest, LinkedIssue, LinkedPullRequest, PullLinkAddress, PullRequestNews } from './pull-request.js'
import type { FolderDiff } from './folder-diff.js'
import type { CloudAgent, CloudFolderState, CloudSetupStep, CloudSignInStep, CloudUse, PublicCloudComputer, PublicCloudTurn } from './cloud-computer.js'
import type { OfficeDocument } from './office-document.js'
import type { Workbook } from './sheet.js'
import type { AboutYouSuggestion } from './about-you.js'
import type { CompareSlotId, PublicCompare, RerunProgress } from './compare.js'
import type { CarryOnResponse, CarryOnRoute, PlanRunsOut } from './carry-on.js'

/**
 * How a teammate's memory is treated: kept at once and shown (the Claude
 * Code way), shown first and kept on a yes (the Cursor way), or not at all.
 */
export type MemoryMode = 'auto' | 'ask' | 'off'
export const DEFAULT_MEMORY_MODE: MemoryMode = 'auto'

/** One thing the team remembers, with who wrote it, where, and from what. */
export interface PublicMemory {
  readonly memoryId: string
  readonly text: string
  readonly scope: MemoryScope
  /** The folder it was written in (`ws_` id); a global memory still records where it came from. */
  readonly workspaceId: string
  readonly workspaceName: string
  readonly by: { readonly teammateId?: string; readonly name: string }
  readonly missionId?: string
  readonly createdAt: string
  /** `proposed` waits for the person; only `kept` is briefed. */
  readonly status: 'kept' | 'proposed'
  readonly enabled: boolean
  /**
   * The slug this memory is filed under, when it has one.
   *
   * A named memory is REPLACEABLE: remembering the same name in the same
   * place rewrites this one instead of adding a near-copy beside it. Absent
   * on every memory written before 0.242 and on every unnamed one since,
   * which keeps matching by text exactly as it was.
   */
  readonly name?: string
  /** When a named memory was last rewritten. Absent if it never has been. */
  readonly updatedAt?: string
  /**
   * Who wrote its CURRENT wording, when it has been rewritten (A1.6). `by`
   * is who first kept it. A rewrite used to leave only `by`, so Booty's
   * "Deploys go out on Thursdays" was credited to the person who had
   * written "Fridays" -- on the Memory screen, on the card, in every brief.
   */
  readonly updatedBy?: { readonly teammateId?: string; readonly name: string }
  /**
   * On a PROPOSAL only: the kept memory this one would REPLACE, or REMOVE,
   * once the person keeps it. "Ask me first" covers rewrites and forgets
   * (0.315); the kept memory stays as it is, and briefed, until then.
   */
  readonly replaces?: string
  readonly forgets?: string
  /** When a teammate was last given it in a brief (A1.4); written at most once a day. */
  readonly lastBriefedAt?: string
  /** On a PROPOSAL only: the kept memories this one would MERGE into one, the first keeping its place (A1.2). */
  readonly merges?: readonly string[]
  /** Why a proposal asks what it asks, when its writer said -- a tidy pass's reason to retire (A1.2). */
  readonly reason?: string
  /**
   * The words of what a proposal would change, as they were when it was
   * made, as a fingerprint. Keeping it after any of them changed is refused
   * (A1.2, agent-native's hash gate): the suggestion was about words that
   * are no longer there.
   */
  readonly basis?: string
  /**
   * What this memory said before it was last rewritten.
   *
   * One step, not a chain: these are single lines of at most 300 characters,
   * and the file has a size cliff that a full history would walk into. It is
   * enough to see what changed and to put it back by hand, which is what a
   * person actually does with a note.
   */
  readonly previousText?: string
}

/**
 * A kept memory that was forgotten, held for a Restore (A1.8): 7 days, then
 * gone for good. Wrapped rather than flattened, so nothing that takes a live
 * memory can be handed a forgotten one by accident.
 */
export interface PublicForgottenMemory {
  readonly memory: PublicMemory
  readonly forgottenAt: string
  /** "you", or the teammate who forgot it -- or asked to, and was agreed with. */
  readonly forgottenBy: { readonly teammateId?: string; readonly name: string }
}

export interface MemoryAddRequest {
  readonly text: string
  readonly scope: MemoryScope
}

export interface MemoryUpdateRequest {
  readonly memoryId: string
  readonly text?: string
  readonly enabled?: boolean
  /** Keep a proposed memory. */
  readonly keep?: boolean
}

export interface MemoryClearRequest {
  readonly scope: 'workspace' | 'all'
}

/**
 * What a runtime has configured for itself, read from its own files and
 * never changed: MCP server names and hook events with their counts.
 * `sources` are the files read; `unreadable` the ones that exist but could
 * not be read or parsed. Names only -- no commands, no arguments.
 */
export interface PublicRuntimeSetup {
  readonly mcpServers: readonly string[]
  readonly hooks: readonly string[]
  /** Skills and agents the runtime can reach for, by name; Claude Code today. */
  readonly skills: readonly string[]
  readonly agents: readonly string[]
  readonly sources: readonly string[]
  readonly unreadable: readonly string[]
}

/** The folder's LOCUST.md as the host last read it: how much, and whether the rest was cut. Null when there is none. */
export interface PublicWorkspaceBrief {
  readonly lines: number
  readonly truncated: boolean
}

/** A teammate's own worktree, as git reports it. */
export interface PublicWorktree {
  readonly teammateId: string
  readonly teammateName: string | undefined
  readonly branch: string
  readonly path: string
  /** A run is live in it, so it cannot be removed now. */
  readonly busy: boolean
}

export type WorktreeListResponse =
  | { readonly ok: true; readonly data: { readonly worktrees: readonly PublicWorktree[]; readonly reason: string | undefined } }
  | { readonly ok: false; readonly error: { readonly code: 'WORKTREES_UNAVAILABLE'; readonly message: string } }
  /** C1: the copy has uncommitted changes; removing it would delete these. */
  | { readonly ok: false; readonly error: { readonly code: 'WORKTREE_HAS_CHANGES'; readonly message: string; readonly changes: readonly string[] } }

export type RuntimeSetupResponse =
  | {
      readonly ok: true
      readonly data: { readonly runtimes: Readonly<Record<string, PublicRuntimeSetup>>; readonly workspaceBrief: PublicWorkspaceBrief | null }
    }
  | { readonly ok: false; readonly error: { readonly code: 'SETUP_UNAVAILABLE'; readonly message: string } }

export type MemoryListResponse =
  | {
      readonly ok: true
      readonly data: {
        readonly memories: readonly PublicMemory[]
        /** Recently forgotten, newest first (A1.8). */
        readonly forgotten?: readonly PublicForgottenMemory[]
        /** By memory id: the files it names that changed after it was written (A1.3). */
        readonly changedSince?: Readonly<Record<string, readonly string[]>>
        /** When Locust began counting which memories teammates are given (A1.4). */
        readonly briefTrackingSince?: string
        readonly workspaceId: string
        readonly workspaceName: string
      }
    }
  | { readonly ok: false; readonly error: { readonly code: 'MEMORY_REJECTED'; readonly message: string } }

export const RUNTIME_DISCOVERY_CHANNEL = 'runtime-discovery:get'
/** Incremental discovery, pushed as it happens rather than answered once. */
export const RUNTIME_DISCOVERY_EVENT_CHANNEL = 'runtime-discovery:event'
/** The backlog, for a renderer that mounts mid-sweep. */
export const RUNTIME_DISCOVERY_LOG_CHANNEL = 'runtime-discovery:log'
/** The loading window saying it is finished, so the app may open. */
export const SPLASH_DONE_CHANNEL = 'splash:done'

/**
 * What discovery is doing, while it is doing it.
 *
 * The boot screen is a view of THESE, not of a timer: nothing on it is
 * invented and no line is typed out that the app already has in full.
 *
 * `probe.started` is emitted before the subprocess is spawned. That is the
 * contract, not an implementation detail -- the screen exists to show the
 * gap between issuing a command and hearing back, and if both arrive
 * together there is nothing to show during the wait.
 */
export type DiscoveryEvent =
  | { readonly kind: 'started'; readonly at: number }
  /*
   * Named runtimes asked again on their own -- a return to the window, or a
   * run starting on a runtime whose answer had gone stale. Not a sweep, and
   * not narrated by the boot screen; recorded so the log says what was asked.
   */
  | { readonly kind: 'reasked'; readonly ids: readonly string[]; readonly at: number }
  | {
      readonly kind: 'context'
      readonly version: string
      readonly platform: string
      readonly workspace: string
      readonly branch?: string
      readonly clean?: boolean
      readonly ledgerPath: string
      readonly ledgerOk: boolean
    }
  /*
   * `id` is the KEY and `bin` is what is shown.
   *
   * They are not the same string and that cost a real defect: `started`
   * carried the command name and `finished` carried the runtime id, so for
   * every runtime where those differ -- `cursor-agent` against `cursor` --
   * the result never found its row. Colin watched Cursor sit on "still
   * waiting" and then appear green in the table underneath (2026-09-14).
   * One key, named once, on both events.
   */
  | {
      readonly kind: 'probe.started'
      readonly id: string
      readonly bin: string
      readonly product: string
      readonly at: number
    }
  | {
      readonly kind: 'probe.finished'
      readonly id: string
      readonly at: number
      readonly outcome: 'missing' | 'needs-signin' | 'ready' | 'error'
      readonly version?: string
      readonly detail?: string
    }
  | { readonly kind: 'finished'; readonly at: number; readonly ready: number; readonly needsYou: number }
export const CODEX_MISSION_START_CHANNEL = 'codex-mission:start'
export const CODEX_MISSION_CANCEL_CHANNEL = 'codex-mission:cancel'
export const MISSION_HANDOFF_CHANNEL = 'mission:hand-off'
export const MISSION_RESUME_CHANNEL = 'mission:resume'
export const CODEX_MISSION_UPDATE_CHANNEL = 'codex-mission:update'
export const MISSION_HISTORY_CHANNEL = 'mission-history:list'
/** H3: one mission's record, whole -- for a conversation history listed as a row only. */
export const MISSION_READ_CHANNEL = 'mission-history:read'
export const MISSION_DELETE_CHANNEL = 'mission:delete'
export const APP_INFO_CHANNEL = 'app:info'
/** 0.379: how many things need the person, for the taskbar's dot and flash. */
export const NEEDS_YOU_COUNT_CHANNEL = 'attention:needs-you-count'
/** 0.379: a long run the person started finished; main says it if the window is elsewhere. */
export const RUN_FINISHED_CHANNEL = 'attention:run-finished'
/**
 * SINCE YOU WERE AWAY (0.590, shared/away.ts). The main process keeps the moment
 * the person was last at the window; the renderer asks for it, is told when a
 * new absence ends, hands back the counts for the tray, and says when the list
 * was seen.
 */
export const AWAY_SINCE_GET_CHANNEL = 'away:since'
export const AWAY_SINCE_CHANNEL = 'away:since-changed'
export const AWAY_SUMMARY_CHANNEL = 'away:summary'
export const AWAY_SEEN_CHANNEL = 'away:seen'
/** 0.379: a finished run's toast was clicked -- open its conversation. */
export const ATTENTION_OPEN_MISSION_CHANNEL = 'attention:open-mission'
export const APP_CHANGELOG_CHANNEL = 'app:changelog'
/** The banner was drawn: this version is seen. Not when the changelog was read. */
export const APP_CHANGELOG_SEEN_CHANNEL = 'app:changelog-seen'

/** One group of a build's changes -- "New", "Improved", "Fixed" -- or, before the groups, the whole entry. */
export interface AppChangelogGroup {
  readonly label?: string
  /** Markdown. */
  readonly text: string
}

/** One build in What's new. */
export interface AppChangelogEntry {
  readonly version: string
  readonly date?: string
  readonly groups: readonly AppChangelogGroup[]
}

/** What changed in the build that is running. */
export interface AppChangelog {
  readonly version: string
  /** The entry's markdown, without its heading; absent when the file has none for this build. */
  readonly body?: string
  readonly date?: string
  /**
   * This is the first launch on this version -- so the window may say what
   * changed unprompted. Decided by the host, once per launch, against a
   * version it remembers; the window has nowhere durable to keep that.
   */
  readonly firstRun: boolean
  /** Every build the changelog that shipped with this one describes, newest first. */
  readonly entries?: readonly AppChangelogEntry[]
  /**
   * The builds marked big since the one this person last saw -- the home
   * screen's splash, once. Empty on most updates, and on a first install.
   */
  readonly splash?: readonly AppChangelogEntry[]
}
export const MISSION_STORAGE_CHANNEL = 'mission:storage'
export const APP_UPDATE_CHECK_CHANNEL = 'app:update-check'
export const APP_UPDATE_INSTALL_CHANNEL = 'app:update-install'
export const APP_UPDATE_STATE_CHANNEL = 'app:update-state'

/**
 * Where an update stands. `unsupported` is a real answer: a development build
 * or one installed outside its installer cannot replace itself, and saying
 * "up to date" there would be a claim nothing checked.
 */
export type AppUpdatePhase =
  | 'idle'
  | 'checking'
  | 'current'
  | 'available'
  | 'downloading'
  | 'ready'
  | 'failed'
  | 'unsupported'

export interface AppUpdateState {
  readonly phase: AppUpdatePhase
  readonly currentVersion: string
  /** Every build as it is published, or only the one a day testers get (main/update-lane.ts). */
  readonly everyBuild?: boolean
  readonly availableVersion?: string
  /**
   * Where to get `availableVersion` by hand: set for a copy the system's package manager owns, a Linux .deb or
   * .rpm, which Locust never replaces itself (0.789, main/linux-install.ts).
   */
  readonly downloadUrl?: string
  readonly percent?: number
  /** Set only when the phase is `failed`; never the provider's own text. */
  readonly message?: string
}

export type AppUpdateResponse =
  | { readonly ok: true; readonly data: AppUpdateState }
  | {
      readonly ok: false
      readonly error: { readonly code: 'UPDATE_NOT_READY' | 'UPDATE_BUSY' | 'INTERNAL_ERROR'; readonly message: string }
    }
/** Switch between every build and the one a day testers get; answers with the update state. */
export const APP_UPDATE_LANE_CHANNEL = 'app-update:lane'
/** What Windows will do at sign-in, read back from the login item itself. */
export const LOGIN_ITEM_GET_CHANNEL = 'login-item:get'
export const LOGIN_ITEM_SET_CHANNEL = 'login-item:set'
/** Keep the process up when the window closes. Off unless the person turned it on. */
export const KEEP_RUNNING_GET_CHANNEL = 'keep-running:get'
export const KEEP_RUNNING_SET_CHANNEL = 'keep-running:set'

export interface LoginItemState {
  readonly openAtLogin: boolean
  /** False in a development build and off Windows. */
  readonly available: boolean
  /** Which of those it is (0.587), so the switch's note can say so. */
  readonly why?: 'development' | 'platform'
}

export interface KeepRunningState {
  readonly keepRunning: boolean
}
export const MISSION_PRUNE_CHANNEL = 'mission:prune'
export const MISSION_TRASH_LIST_CHANNEL = 'mission:trash:list'
export const MISSION_RESTORE_CHANNEL = 'mission:restore'
export const MISSION_TRASH_EMPTY_CHANNEL = 'mission:trash:empty'

/**
 * A conversation that was deleted and is being kept until the trash is
 * emptied. Deleting takes it out of every listing at once; this is what is
 * still on disk behind that.
 */
export interface PublicTrashedMission {
  readonly missionId: string
  readonly deletedAt: string
  readonly bytes: number
  /** The first line of what was asked, so the list can name it. */
  readonly prompt?: string
  readonly createdAt?: string
}

export type TrashListResponse =
  | { readonly ok: true; readonly data: { readonly missions: readonly PublicTrashedMission[] } }
  | { readonly ok: false; readonly error: { readonly code: 'STORAGE_UNAVAILABLE'; readonly message: string } }

export type TrashMutationResponse =
  | { readonly ok: true; readonly data: { readonly count: number } }
  | {
      readonly ok: false
      readonly error: { readonly code: 'RESTORE_REFUSED' | 'INTERNAL_ERROR'; readonly message: string }
    }

/** What the local mission history costs on this machine. */
export interface PublicStorageReport {
  readonly missionCount: number
  readonly byteTotal: number
  readonly oldestUpdatedAt?: string
}

export type StorageReportResponse =
  | { readonly ok: true; readonly data: PublicStorageReport }
  | { readonly ok: false; readonly error: { readonly code: 'STORAGE_UNAVAILABLE'; readonly message: string } }

export interface MissionPruneRequest {
  /** Missions untouched for longer than this are candidates. */
  readonly olderThanDays: number
  /** Ask what would happen. The preview and the deletion are one code path. */
  readonly dryRun: boolean
  /**
   * On a confirm: the missions its preview showed, and no others (L8). A
   * preview left open while missions crossed the cutoff, or finished, used
   * to delete those too.
   */
  readonly only?: readonly string[]
}

export interface MissionPruneData {
  readonly deleted: readonly string[]
  /** Old missions kept because a mission that survives continues from them. */
  readonly keptForContinuity: readonly string[]
  /** Old missions kept because they are running right now. */
  readonly keptAsRunning: readonly string[]
  /** Whether this was a preview. A preview deleted nothing. */
  readonly previewed: boolean
}

export type MissionPruneResponse =
  | { readonly ok: true; readonly data: MissionPruneData }
  | {
      readonly ok: false
      readonly error: { readonly code: 'PRUNE_REFUSED' | 'INTERNAL_ERROR'; readonly message: string }
    }

/*
 * BACK UP AND RESTORE (0.614, main/profile-backup.ts): a folder with a
 * manifest, keys never in it; a restore previewed by the same reader, refused
 * for a newer backup or a live run, and applied at the next start.
 */
export const PROFILE_PICK_FOLDER_CHANNEL = 'profile:pick-folder'
export const PROFILE_BACKUP_CHANNEL = 'profile:backup'
export const PROFILE_RESTORE_PREVIEW_CHANNEL = 'profile:restore-preview'
export const PROFILE_RESTORE_CHANNEL = 'profile:restore'
export const PROFILE_LAST_RESTORE_CHANNEL = 'profile:last-restore'

/** What a profile, or a backup of one, holds, in the person's own nouns. */
export interface ProfileCounts {
  readonly teammates: number
  readonly routines: number
  readonly memories: number
  readonly rules: number
  readonly groups: number
  readonly rooms: number
  readonly compares: number
  readonly folders: number
  readonly conversations: number
}

export type ProfileBackupResponse =
  | { readonly ok: true; readonly folder: string; readonly counts: ProfileCounts; readonly bytes: number; readonly files: number }
  | { readonly ok: false; readonly reason: string }

export type ProfileRestorePreview =
  | {
      readonly ok: true
      readonly folder: string
      /** The Locust that made it, and when. */
      readonly appVersion: string
      readonly createdAt: string
      readonly counts: ProfileCounts
      readonly bytes: number
      /** What is here now, which the restore would replace. */
      readonly current: ProfileCounts
    }
  | { readonly ok: false; readonly reason: string }

export type ProfileRestoreResponse = { readonly ok: true } | { readonly ok: false; readonly reason: string }

/** What the last restore did, said once after the restart it needed. */
export interface ProfileRestoreOutcome {
  readonly ok: boolean
  readonly folder: string
  readonly at: string
  /** Where what it replaced was moved, inside the profile. */
  readonly aside?: string
  readonly counts?: ProfileCounts
  readonly reason?: string
}

/** What this build is, so a person can say which one they are running. */
export interface AppInfo {
  /** The main process's launch-time, machine-wide policy; never a renderer preference. */
  readonly organizationPolicy?: import('./organization-policy.js').OrganizationPolicyState
  readonly name: string
  readonly version: string
  readonly packaged: boolean
  /**
   * The platform this build is running on. Some containment is platform
   * specific -- Cursor's sandbox exists on macOS and Linux only -- and the
   * window has to know, or it offers modes the host will refuse.
   */
  readonly platform: string
  /**
   * The folder every mission runs in, by its last segment; empty when none
   * is chosen. The folder is the one the app was launched from, or the one
   * chosen last time when it was launched from its own install folder --
   * never the install folder itself.
   */
  readonly workspaceName: string
  readonly workspacePath: string
  /**
   * Locust made the folder itself (Documents\Locust) because nothing was
   * chosen and the app was launched from its install folder. The window
   * says so where the folder is named, so a person knows where their
   * teammates' files went and that any other folder is one click away.
   */
  readonly workspaceMade: boolean
  /**
   * The folder is no place for teammates (0.790): "your home folder", or ".claude, where the agents keep their own
   * settings" (main/workspace.ts notATeammateFolder). The folder chip says so, in amber.
   */
  readonly workspaceCaution?: string
  /**
   * Where the durable receipts are written. Carried so that the one card
   * about a ledger that cannot be written can offer to open the folder;
   * nothing else in the renderer needs it.
   */
  readonly ledgerPath: string
}

/**
 * The answer to "choose a folder". A chosen folder makes the app reopen
 * itself there: every service binds the folder when it starts and the
 * mission list is scoped by it, so a running window cannot simply switch.
 */
export type WorkspaceChooseResponse =
  | { readonly ok: true; readonly data: { readonly path: string; readonly id: string; readonly name: string } }
  | {
      readonly ok: false
      readonly error: {
        readonly code: 'CANCELLED' | 'INSTALL_FOLDER' | 'RUNS_ACTIVE' | 'INTERNAL_ERROR'
        readonly message: string
      }
    }
/*
 * PETS AS TEAMMATES' FACES (0.563, main/pet-library.ts). The host keeps them,
 * checks them and reads them; the window only ever names one by its source
 * and id -- never a path, never an address.
 */
/** The pets on this computer: Locust's own, the ones added from the gallery, and Codex's. */
export const PET_LIST_CHANNEL = 'pets:list'
/** One pet's sheet, as bytes, for the window to draw. */
export const PET_SHEET_CHANNEL = 'pets:sheet'
/** The small picture of a pet Locust offers (shared/pet-picks.ts), as a data URL. */
export const PET_THUMBNAIL_CHANNEL = 'pets:thumbnail'
/** Download a gallery pet to this computer: only ever on the person's click. */
export const PET_ADD_CHANNEL = 'pets:add'
/** Take a gallery pet off this computer, refused while a teammate wears it. */
export const PET_REMOVE_CHANNEL = 'pets:remove'
export const TEAMMATE_LIST_CHANNEL = 'teammates:list'
export const TEAMMATE_CREATE_CHANNEL = 'teammates:create'
export const TEAMMATE_REMOVE_CHANNEL = 'teammates:remove'
export const TEAMMATE_UPDATE_CHANNEL = 'teammates:update'
/** What each teammate has spent this calendar month, from every conversation (shared/spend.ts). */
export const TEAMMATE_SPEND_CHANNEL = 'teammates:spend'
export const TEAMMATE_ASSIGN_CHANNEL = 'teammates:assign'
/**
 * Give a conversation a name of your own.
 *
 * It rides with the roster rather than the ledger on purpose: the ledger is
 * an append-only record of what was asked and what happened, and a title
 * someone changed afterwards is neither. An empty name clears it, and the
 * conversation falls back to the first line that was typed -- which was
 * never overwritten, so nothing is lost by renaming and nothing is lost by
 * undoing it.
 */
export const TEAMMATE_RENAME_MISSION_CHANNEL = 'teammates:renameMission'
/** Pin a conversation to the top of the sidebar, or unpin it (0.729). */
export const TEAMMATE_PIN_MISSION_CHANNEL = 'teammates:pinMission'
/** Settle a conversation out of the sidebar, snooze it until a time, or bring it back (0.730). */
export const TEAMMATE_SETTLE_MISSION_CHANNEL = 'teammates:settleMission'
export const ROUTINE_LIST_CHANNEL = 'routines:list'
export const ROUTINE_CREATE_CHANNEL = 'routines:create'
export const ROUTINE_UPDATE_CHANNEL = 'routines:update'
export const ROUTINE_REMOVE_CHANNEL = 'routines:remove'
export const ROUTINE_PAUSE_CHANNEL = 'routines:pause'
export const ROUTINE_RUN_CHANNEL = 'routines:run'
/**
 * A routine that asks for inputs (W7): a folder value only ever comes from this
 * picker, and a routine travels as a `.locust-routine.json` file -- written by
 * Export, read (as a preview that starts nothing) and then added by Import.
 */
export const ROUTINE_FOLDER_CHANNEL = 'routines:chooseFolder'
export const ROUTINE_EXPORT_CHANNEL = 'routines:export'
export const ROUTINE_IMPORT_PREVIEW_CHANNEL = 'routines:importPreview'
export const ROUTINE_IMPORT_CHANNEL = 'routines:import'
/** The starter routines (0.615, main/routine-templates.ts): the list, and one previewed like an imported file. */
export const ROUTINE_TEMPLATES_CHANNEL = 'routines:templates'
export const ROUTINE_TEMPLATE_PREVIEW_CHANNEL = 'routines:templatePreview'
/**
 * A routine that works in a copy (0.533): its last run's changes, kept (written
 * into the folder), discarded, or the copy opened to look at first.
 */
export const ROUTINE_SETTLE_CHANNEL = 'routines:settle'
export interface RoutineSettleRequest {
  readonly routineId: string
  readonly decision: 'keep' | 'discard' | 'open'
}
export type RoutineSettleResponse = { readonly ok: true; readonly message: string } | { readonly ok: false; readonly message: string }
export const MODEL_CATALOG_CHANNEL = 'models:list'
/**
 * What the person set up inside the CLIs themselves -- agents, commands and
 * automations they wrote for Claude Code, Codex or Cursor. Locust neither
 * creates nor runs these; it lists them, because a machine with nine of them
 * on it read "Nothing saved yet" (Colin, 2026-09-07).
 */
export const RUNTIME_ARTIFACTS_CHANNEL = 'runtime-artifacts:list'
export interface PublicRuntimeArtifact {
  readonly runtime: string
  readonly kind: 'agent' | 'command' | 'automation'
  readonly name: string
  readonly description?: string
  readonly path: string
}

/**
 * Installing a runtime the person asked for, from inside the app.
 *
 * Colin, 2026-09-06: "why would you not want to give the user the ability to
 * click something to get what they would need to make it work". The app shows
 * the exact command before it runs it, runs one at a time, and streams npm's
 * own output back so a slow install does not look like a frozen one.
 */
export const RUNTIME_INSTALL_CHANNEL = 'runtime:install'
/** npm's output, line by line, while an install runs. */
export const RUNTIME_INSTALL_PROGRESS_CHANNEL = 'runtime:install-progress'

export interface RuntimeInstallProgress {
  readonly runtime: string
  readonly line: string
}

/**
 * Keeping the coding agents current (main/runtime-updates.ts).
 *
 * Colin, 2026-09-23: "is there a way to make it so the models will
 * automatically update without messing up load times or interfering with the
 * app". The models are read live from each agent; the agents that never update
 * themselves -- Codex and Copilot, from npm -- are updated here, after launch
 * and only when nothing is using them.
 */
export const RUNTIME_UPDATES_CHANNEL = 'runtime-updates:read'
export const RUNTIME_UPDATES_SET_CHANNEL = 'runtime-updates:set'
/** The person pressed Update on a runtime's row. */
export const RUNTIME_UPDATES_NOW_CHANNEL = 'runtime-updates:now'
/** Pushed when an update starts, lands or fails. */
export const RUNTIME_UPDATES_EVENT_CHANNEL = 'runtime-updates:changed'

/**
 * Undo a turn (0.674): what each finished turn can take back, and taking it back.
 * Only run ids cross the bridge; the folder and what was kept stay in the main process.
 */
export const TURN_UNDO_STATE_CHANNEL = 'turn-undo:state'
export const TURN_UNDO_CHANNEL = 'turn-undo:undo'

export interface TurnUndoLeftAlone {
  readonly path: string
  readonly why: string
}

/** What a turn can say about its way back. */
export type TurnUndoState =
  | { readonly kind: 'none' }
  | { readonly kind: 'shared' }
  | { readonly kind: 'ready'; readonly files: number; readonly notKept: readonly string[] }
  | { readonly kind: 'undone'; readonly at: string; readonly restored: readonly string[]; readonly leftAlone: readonly TurnUndoLeftAlone[] }

export type RuntimeUpdateStatus =
  | { readonly kind: 'current' }
  /**
   * Newer is out and waits: for the person to press Update (updating on its
   * own is off), for nothing to be using it, or -- updating on its own -- for
   * it to have been out long enough to trust.
   */
  /*
   * `held` (0.501): Locust's own check of each new release -- one real turn
   * read by Locust -- failed on this version, so it is not installed on its
   * own. Update still installs it: that is the person's choice to make.
   */
  | { readonly kind: 'waiting'; readonly version: string; readonly why: 'ask' | 'in use' | 'too new' | 'held' }
  | { readonly kind: 'updating'; readonly version: string }
  | { readonly kind: 'updated'; readonly from: string; readonly to: string; readonly at: string }
  | { readonly kind: 'failed'; readonly version: string; readonly what: string; readonly at: string }

export interface RuntimeUpdateView {
  readonly runtime: string
  readonly installed: string
  readonly latest: string | undefined
  readonly status: RuntimeUpdateStatus
}

export interface RuntimeUpdatesState {
  /** Updates without being asked: off unless the person turned it on. */
  readonly automatic: boolean
  /** When npm was last asked; undefined before the first look. */
  readonly checkedAt: string | undefined
  readonly agents: readonly RuntimeUpdateView[]
  /** This launch never updates the machine's agents on its own: a copy, or a scripted run (0.514). */
  readonly heldHere?: true
}

export type RuntimeInstallResponse =
  | { readonly ok: true }
  | {
      readonly ok: false
      /** One sentence: what happened. */
      readonly what: string
      /** One sentence: what to do about it. */
      readonly next: string
      /** Whether restarting Locust is the action, rather than running it again. */
      readonly restart?: boolean
      /**
       * The command to show INSTEAD of the one Locust ran, when the remedy is
       * a different command. A permission failure is the case: re-running the
       * same `npm install -g` fails identically, because the global prefix is
       * what is unwritable.
       */
      readonly command?: string
    }
/**
 * Open a runtime's own sign-in in a window of its own. The renderer sends the
 * runtime id only; main runs the command the install facts name, against the
 * executable discovery found. See `main/runtime-sign-in.ts`.
 */
export const RUNTIME_SIGN_IN_CHANNEL = 'runtime:sign-in'
/** The host says a sign-in window it opened has closed, so the row can recover (0.545). */
export const RUNTIME_SIGN_IN_CLOSED_CHANNEL = 'runtime:sign-in-closed'

export type RuntimeSignInResponse =
  | { readonly ok: true }
  | { readonly ok: false; readonly what: string; readonly next: string }

/**
 * Open a conversation in its runtime's own terminal interface: the same
 * session, in the teammate's folder (0.387). The renderer names a mission;
 * the host finds its session in the ledger, its folder from the roster and
 * its program from discovery. See `main/open-in-terminal.ts`.
 */
export const OPEN_IN_TERMINAL_CHANNEL = 'mission:open-in-terminal'

/**
 * Bring what the person did in a runtime's own terminal on a conversation
 * into its record (0.391; main/terminal-catch-up.ts). The window names the
 * conversation's newest turn; the answer says how many exchanges came back
 * and which turn is newest now.
 */
export const TERMINAL_CATCH_UP_CHANNEL = 'mission:terminal-catch-up'
/**
 * Import a conversation from Claude Code or Codex (main/session-import.ts):
 * the sessions a person could bring in, and bringing one in by its id.
 */
export const SESSION_IMPORT_LIST_CHANNEL = 'session-import:list'
export const SESSION_IMPORT_CHANNEL = 'session-import:import'
export interface ImportableSessionView {
  readonly runtime: 'claude' | 'codex' | 'cursor' | 'opencode'
  readonly sessionId: string
  readonly title: string
  readonly cwd: string
  readonly folderName: string
  readonly updatedAt: string
  readonly bytes: number
  readonly openNow: boolean
  readonly folderMissing?: boolean
}
export type SessionImportListResponse = { readonly ok: true; readonly sessions: readonly ImportableSessionView[]; readonly warnings?: readonly string[] } | { readonly ok: false; readonly message: string }
export type SessionImportResponse = { readonly ok: true; readonly missionId: string; readonly turns: number; readonly skipped: number } | { readonly ok: false; readonly message: string }
/** The team as a picture of itself (shared/team-card.ts, 0.398). */
export const TEAM_CARD_SAVE_CHANNEL = 'team-card:save'
export const TEAM_CARD_ADD_CHANNEL = 'team-card:add'
/**
 * Save the record (0.575): one conversation as one Markdown file, written
 * where the person picks, and optionally the events exactly as the ledger
 * holds them beside it. Nothing uploads and nothing opens.
 */
export const MISSION_RECORD_SAVE_CHANNEL = 'missions:save-record'
/** Where on the window the card is drawn, in CSS pixels: what is photographed. */
export interface TeamCardRect {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}
/** `path` absent: the person cancelled the save, which is not a failure. */
export type TeamCardSaveResponse = { readonly ok: true; readonly path?: string } | { readonly ok: false; readonly message: string }
/** Who was added, and who could not be (by the name the card gave them). Nothing picked: both empty. */
export type TeamCardAddResponse =
  | {
      readonly ok: true
      readonly added: readonly string[]
      readonly skipped: readonly string[]
      /** Who came with instructions of their own (0.802): someone else's words, so the window says where to read them. */
      readonly instructed: readonly string[]
      /** The card's routines made here, paused (0.802), and those that were not, each with why. */
      readonly routines: readonly string[]
      readonly routinesLeft: readonly string[]
    }
  | { readonly ok: false; readonly message: string }

export interface MissionRecordSaveRequest {
  /** Any turn of the conversation; its parents come with it. */
  readonly missionId: string
  /** Also write the events exactly as the ledger holds them, as JSON beside the file. */
  readonly includeRaw: boolean
}
/** `path` absent: the person cancelled the save, which is not a failure. */
export type MissionRecordSaveResponse =
  | { readonly ok: true; readonly path?: string; readonly rawPath?: string }
  | { readonly ok: false; readonly message: string }

export interface TerminalCatchUpResponse {
  readonly imported: number
  readonly latestMissionId?: string
  /** The turns just recorded, by the teammate they belong to (terminal-catch-up.ts). */
  readonly owners?: Readonly<Record<string, string>>
}

export type OpenInTerminalResponse =
  /** Where it opened, as the person calls it: Windows Terminal, a console window, Terminal, or a Linux terminal's name. */
  | { readonly ok: true; readonly where: string }
  | { readonly ok: false; readonly message: string }
export const WORKSPACE_SETTINGS_READ_CHANNEL = 'workspace-settings:read'
export const WORKSPACE_SETTINGS_WRITE_CHANNEL = 'workspace-settings:write'
export const WORKSPACE_CHOOSE_CHANNEL = 'workspace:choose'
/** Start here, on the window's question about a folder like home or `.claude` (0.790): the current folder is kept. */
export const WORKSPACE_CONFIRM_CHANNEL = 'workspace:confirm'
/** Every folder worked in, and the window's (0.458). */
export const FOLDER_LIST_CHANNEL = 'folder:list'
/** Ask a question on the side of a conversation: read-only, on a copy of its session (0.461). */
export const SIDE_ASK_CHANNEL = 'side:ask'
/**
 * CLOUD TASKS (0.503, main/cloud-tasks.ts): work handed to Codex Cloud from
 * the chat-type menu, followed here, and applied only when the person asks.
 */
export const CLOUD_WHERE_CHANNEL = 'cloud:where'
export const CLOUD_START_CHANNEL = 'cloud:start'
export const CLOUD_LIST_CHANNEL = 'cloud:list'
export const CLOUD_REFRESH_CHANNEL = 'cloud:refresh'
export const CLOUD_DIFF_CHANNEL = 'cloud:diff'
export const CLOUD_APPLY_CHANNEL = 'cloud:apply'
/** The person's folders that are on GitHub, for a cloud task from a folder that is not (0.504). */
export const CLOUD_FOLDERS_CHANNEL = 'cloud:folders'
/** Your cloud computer (0.761): a conversation's turns there, their changes, and the computers themselves. */
export const CLOUD_TURNS_CHANNEL = 'cloud-turn:list'
export const CLOUD_TURN_PATCH_CHANNEL = 'cloud-turn:patch'
export const CLOUD_TURN_APPLY_CHANNEL = 'cloud-turn:apply'
export const CLOUD_TURN_DISCARD_CHANNEL = 'cloud-turn:discard'
export const CLOUD_TURN_PULL_REQUEST_CHANNEL = 'cloud-turn:pull-request'
export const CLOUD_COMPUTERS_CHANNEL = 'cloud-computer:list'
export const CLOUD_COMPUTER_STOP_CHANNEL = 'cloud-computer:stop'
export const CLOUD_COMPUTER_REMOVE_CHANNEL = 'cloud-computer:remove'
export const CLOUD_COMPUTER_TERMINAL_CHANNEL = 'cloud-computer:terminal'
export const CLOUD_COMPUTER_ALLOW_CHANNEL = 'cloud-computer:allow'
export const CLOUD_COMPUTER_FOLDER_CHANNEL = 'cloud-computer:folder'
export const CLOUD_COMPUTER_SETUP_CHANNEL = 'cloud-computer:set-up'
/** Claude Code or Codex signed in on a cloud computer, in the person's own browser (0.768); stopped; signed out. */
export const CLOUD_COMPUTER_SIGN_IN_CHANNEL = 'cloud-computer:sign-in'
export const CLOUD_COMPUTER_SIGN_IN_STOP_CHANNEL = 'cloud-computer:sign-in-stop'
export const CLOUD_COMPUTER_SIGN_OUT_CHANNEL = 'cloud-computer:sign-out'
/**
 * CLAUDE'S CLOUD (0.538, main/claude-cloud.ts): a task handed to a Claude Code
 * cloud session, in a window of its own; listed here so it can be found on
 * claude.ai or brought home with `claude --teleport`.
 */
export const CLAUDE_CLOUD_START_CHANNEL = 'claude-cloud:start'
export const CLAUDE_CLOUD_LIST_CHANNEL = 'claude-cloud:list'
export const CLAUDE_CLOUD_HOME_CHANNEL = 'claude-cloud:home'
export const CLAUDE_CLOUD_FORGET_CHANNEL = 'claude-cloud:forget'
export const CLAUDE_CLOUD_SEND_CHANNEL = 'claude-cloud:send'
export const CLAUDE_CLOUD_CHECK_CHANNEL = 'claude-cloud:check'
export const CLAUDE_CLOUD_APPLY_CHANNEL = 'claude-cloud:apply'
export const CLAUDE_CLOUD_TERMINAL_CHANNEL = 'claude-cloud:terminal'
export const CLAUDE_CLOUD_ENVIRONMENT_CHANNEL = 'claude-cloud:environment'
export interface ClaudeCloudSession {
  readonly id: string
  readonly startedAt: string
  /** The task as it was sent, bounded. */
  readonly prompt: string
  readonly folder: string
  readonly teammateId?: string
  /** Claude Code's own id for it, when Locust started it out of sight (0.556). */
  readonly sessionId?: string
  /** The session on claude.ai. */
  readonly url?: string
  /** The title Claude Code gave it. */
  readonly title?: string
  /** What Claude Code said about where it starts from, or what it waits for. */
  readonly note?: string
}
export type PublicClaudeCloudSession = Omit<ClaudeCloudSession, 'folder'>
export type ClaudeCloudStartResponse =
  | { readonly ok: true; readonly session: PublicClaudeCloudSession }
  | { readonly ok: false; readonly message: string }
export type ClaudeCloudHomeResponse = { readonly ok: true } | { readonly ok: false; readonly message: string }
/** One exchange of a cloud session, read from the transcript Claude Code saved when it brought the session in (0.558). */
export interface ClaudeCloudExchange {
  readonly prompt: string
  readonly answer?: string
  readonly at: string
  readonly model?: string
}
export type ClaudeCloudReading =
  | {
      readonly ok: true
      readonly exchanges: readonly ClaudeCloudExchange[]
      readonly checkedAt: string
      /** The session branch's change against the folder's commit, as a unified diff. */
      readonly diff?: string
      /** There is a change, too big to show here; Apply still brings it in. */
      readonly changeTooBig?: boolean
      readonly note?: string
    }
  | { readonly ok: false; readonly message: string }
export interface PublicCloudFolder {
  readonly id: string
  readonly name: string
  readonly path: string
  readonly repo: string
  readonly environment: 'ready' | 'missing' | 'unknown'
}
export interface PublicCloudTask {
  readonly taskId: string
  readonly url: string
  readonly runtime: 'codex'
  readonly repo: string
  readonly branch: string | undefined
  readonly prompt: string
  readonly createdAt: string
  readonly teammateId?: string
  readonly status: {
    readonly state: 'pending' | 'ready' | 'failed' | 'applied' | 'unknown'
    readonly title: string | undefined
    readonly added: number | undefined
    readonly removed: number | undefined
    readonly files: number | undefined
  }
  readonly appliedAt?: string
}
/** Where this folder stands for a cloud task: a GitHub repository, its branch, what GitHub lacks. */
export type CloudTurnResponse = { readonly ok: true; readonly turn: PublicCloudTurn } | { readonly ok: false; readonly message: string }
export type CloudPullRequestResponse = { readonly ok: true; readonly turn: PublicCloudTurn; readonly added: boolean } | { readonly ok: false; readonly message: string }
/** An agent's sign-in on a cloud computer (0.768): `permission` when GitHub's own permission is what is missing. */
export type CloudSignInResponse = { readonly ok: true } | { readonly ok: false; readonly message: string; readonly permission?: true }

export type CloudComputersResponse =
  /** `use`: Locust's own hours on its cloud computers this month (0.769), once it has spent any. */
  | { readonly ok: true; readonly computers: readonly PublicCloudComputer[]; readonly use?: CloudUse }
  | { readonly ok: false; readonly message: string; readonly permission?: true }
export interface PublicCloudWhere {
  readonly repo: string | undefined
  readonly branch: string | undefined
  readonly unpushed: number | undefined
  readonly dirty: boolean
  /** Codex CLI is here and signed in: the cloud is reached through it. */
  readonly codexReady: boolean
  /** The folder's own name, so a refusal can say which folder (0.504). */
  readonly folderName?: string
  /** Whether Codex Cloud has an environment for its repository, asked before a send (0.505). */
  readonly environment?: 'ready' | 'missing' | 'unknown'
}
export type CloudStartResponse =
  | { readonly ok: true; readonly task: PublicCloudTask; readonly notes: readonly string[] }
  | { readonly ok: false; readonly message: string }
export type CloudApplyResponse =
  | { readonly ok: true; readonly task: PublicCloudTask }
  | { readonly ok: false; readonly message: string }
export type SideAskResponse =
  | { readonly ok: true; readonly data: { readonly runId: string; readonly missionId: string } }
  | { readonly ok: false; readonly message: string }
/** Switch the window to a folder already worked in, by id -- no restart (0.458). */
export const FOLDER_SWITCH_CHANNEL = 'folder:switch'
export interface PublicFolder {
  readonly id: string
  readonly path: string
  readonly name: string
  readonly lastUsedAt?: string
  /** A Locust install (early builds ran there): listed so its old conversations are named, never offered to work in. */
  readonly installFolder?: true
}
export interface FolderListResponse {
  readonly folders: readonly PublicFolder[]
  readonly currentId: string | undefined
}
export type FolderSwitchResponse =
  | { readonly ok: true; readonly folder: { readonly id: string; readonly path: string; readonly name: string } }
  | { readonly ok: false; readonly message: string }
/**
 * Put back what the replies after an edited message changed (0.502,
 * shared/reverse-diff.ts): exactly, or the file is left as it is and named.
 */
export const REWIND_PUT_BACK_CHANNEL = 'rewind:put-back'
export interface RewindPutBackRequest {
  readonly files: readonly {
    readonly path: string
    readonly changes: readonly ReverseChange[]
    readonly cannot?: string
  }[]
}
export interface RewindPutBackResponse {
  readonly putBack: readonly string[]
  readonly leftAlone: readonly { readonly path: string; readonly why: string }[]
  /**
   * Files that could have been put back and were not, because others could
   * not (0.512): all of them or none, so a project is never half put back.
   */
  readonly heldBack?: readonly string[]
}
/**
 * Show a file a teammate wrote, in the operating system's file manager.
 *
 * The renderer sends a path it already holds; the host honours it only if it
 * resolves inside a workspace the host knows about. See `main/reveal-file.ts`
 * for why that check is the whole point of the channel.
 */
export const WORKSPACE_REVEAL_CHANNEL = 'workspace:reveal'
/**
 * Save a COPY of a file a teammate wrote, wherever the person says.
 *
 * Colin, 2026-09-20, looking at a handover card: give it the little download
 * icon Claude Code has, in case the user wants to easily move it to another
 * folder.
 *
 * It is the same containment as a reveal on the way IN -- the source must
 * resolve inside a folder the host already knows a mission ran in, so a model
 * cannot hand over C:/Windows/something and have the app copy it out. The way
 * OUT is a native save dialog, so the destination is the person's own choice
 * and never a path the renderer named.
 *
 * And it is still not opening. Bytes are copied; nothing is executed, and the
 * rule in reveal-file.ts is untouched.
 */
export const WORKSPACE_SAVE_COPY_CHANNEL = 'workspace:save-copy'

/**
 * Show the diagnostics log, so a person can send it.
 *
 * Takes NO path. `workspace:reveal` has to check the renderer's path against
 * the folders a mission ran in, because the renderer proposes the
 * destination; here the host already knows the only answer -- its own profile
 * -- so there is nothing to propose and nothing to check. The rule the app
 * runs under is that the renderer names no destinations, and the strongest
 * form of that is a channel with no argument at all.
 */
export const DIAGNOSTICS_REVEAL_CHANNEL = 'diagnostics:reveal'
/** Open the folder teammates work in, in the file manager (0.797): the host's own folder, never a path the window names. */
export const WORKSPACE_OPEN_CHANNEL = 'workspace:open'

/**
 * Send feedback: the words from the Send feedback box, and the conversation
 * it was sent from when there was one (main/report-problem.ts). The renderer
 * hands over WORDS; the host builds the one address itself, so nothing here
 * can name a destination.
 */
export const FEEDBACK_CHANNEL = 'diagnostics:feedback'
/** The private way (0.593, R23): the same report as an email to SUPPORT_ADDRESS, or saved whole as a file. */
export const FEEDBACK_EMAIL_CHANNEL = 'diagnostics:feedback-email'
export const FEEDBACK_SAVE_CHANNEL = 'diagnostics:feedback-save'

/** What the Send feedback box hands the host. */
export interface FeedbackReport {
  /** What the person wrote. */
  readonly description: string
  /** The conversation it was sent from, as plain text, when it was sent from one. */
  readonly conversation?: string
}

/** Where the log is and whether anything has been written to it yet. */
export interface DiagnosticsReport {
  readonly path: string
  /** False before the first run has written its opening line. */
  readonly exists: boolean
  readonly byteTotal: number
}
export const DIAGNOSTICS_REPORT_CHANNEL = 'diagnostics:report'
/**
 * Choose files to attach to the next message.
 *
 * The host opens the picker and returns only paths INSIDE the workspace --
 * the same containment `main/reveal-file.ts` enforces on the way out, applied
 * on the way in. The renderer never names a folder to open.
 */
/**
 * Attach what is on the clipboard, as bytes.
 *
 * A pasted screenshot has no path -- the clipboard holds a bitmap, not a file
 * -- so there is nothing for the picker's path-based route to take. The
 * renderer sends the bytes and a suggested name; the host decides where they
 * land, exactly as it does for a file chosen from outside the folder.
 */
export const WORKSPACE_PASTE_CHANNEL = 'workspace:paste'
export const WORKSPACE_ATTACH_CHANNEL = 'workspace:attach'
/** The project's files, for `@` in the composer (0.436, main/workspace-files.ts). */
export const WORKSPACE_FILES_CHANNEL = 'workspace:files'

/**
 * Usage across every agent and model (0.714, shared/usage.ts): the host adds
 * up every turn the ledger holds for the range asked, on this machine's clock.
 */
/**
 * An attached PDF's pages, as Locust drew them for the agent (0.714,
 * main/pdf-reading.ts): the chat's file card shows the first, and the viewer
 * shows them all. A PDF not read yet -- one attached before 0.713 -- is read
 * on the way. Folder-relative paths.
 */
export const PDF_PAGES_CHANNEL = 'pdf:pages'
export type PdfPagesResponse =
  | { readonly ok: true; readonly pages: number; readonly pictures: readonly string[]; readonly text?: string }
  | { readonly ok: false; readonly message: string }

export const USAGE_READ_CHANNEL = 'usage:read'
export type UsageReadResponse =
  | { readonly ok: true; readonly summary: UsageSummary }
  | { readonly ok: false; readonly message: string }
export type WorkspaceFilesResponse =
  | { readonly ok: true; readonly paths: readonly string[]; readonly truncated: boolean }
  | { readonly ok: false; readonly message: string }

/** Files chosen to attach, workspace-relative, or why none were. */
export type AttachFilesResponse =
  /**
   * `copied` names the paths that were brought in from outside the workspace,
   * a subset of `paths`. Writing into someone's project folder is not
   * something to do quietly, and this is how the composer says so.
   *
   * It used to be a `message` -- one sentence for the whole batch, drawn as a
   * full-width bordered box above the composer, directly above an actual text
   * input and shaped exactly like one. Two objects for one event, and the
   * transient one was carrying a permanent fact: the file STAYS copied for as
   * long as the tile exists, so dismissing the notice lost information that
   * was still true. The tile is the durable object, so the fact goes there
   * (design, 2026-09-08).
   *
   * A subset, not a flag, because a multi-select can copy some files and not
   * others -- one already inside the folder needs no mark, and marking it
   * would claim something that did not happen.
   */
  | { readonly ok: true; readonly paths: readonly string[]; readonly copied?: readonly string[] }
  | { readonly ok: false; readonly message: string }

export const WORKSPACE_IMAGE_CHANNEL = 'workspace:image'
/**
 * Read a text file a teammate wrote, to show beside the conversation.
 *
 * The same containment as the reveal and the image reader: the path is a
 * REQUEST, honoured only inside a folder the host already knows a mission ran
 * in. Bounded, and a refusal rather than a truncation -- see
 * `shared/text-files.ts` for what is readable and why the list is an
 * allowlist.
 */
export type WorkspaceTextResponse =
  | {
      readonly ok: true
      readonly path: string
      readonly text: string
      /**
       * `table`: a spreadsheet, read for its cells (0.364), carried in `workbook`.
       * `document`: a Word or PowerPoint file, read for its words (0.517), carried in `document`.
       */
      readonly mode: 'markdown' | 'code' | 'table' | 'document'
      readonly workbook?: Workbook
      readonly document?: OfficeDocument
    }
  | { readonly ok: false; readonly message: string }

export const WORKSPACE_TEXT_CHANNEL = 'workspace:text'
/** A web page's address in the preview (0.425): main/page-preview.ts. */
export const WORKSPACE_PAGE_CHANNEL = 'workspace:page'
/** A page a model wrote into its reply, to run on a stage (0.553, shared/reply-page.ts). */
export const REPLY_PAGE_CHANNEL = 'reply:page'
/** Point at a part of a running page (0.484): main/page-pick.ts. */
export const PAGE_PICK_CHANNEL = 'workspace:page-pick'
export const PAGE_PICK_CANCEL_CHANNEL = 'workspace:page-pick-cancel'
export interface PagePickRequest {
  readonly pageUrl: string
  /** The page's frame on screen, in the window's CSS pixels. */
  readonly frame: { readonly x: number; readonly y: number; readonly width: number; readonly height: number }
}
export type PagePickResponse =
  | {
      readonly ok: true
      /** Undefined when the person cancelled, or nothing was picked. */
      readonly picked?: { readonly selector: string; readonly html: string; readonly htmlLength: number }
      /** A picture of the part, as PNG, when any of it was in view. */
      readonly png?: Uint8Array
    }
  | { readonly ok: false; readonly message: string }
/** Each runtime's own slash commands, for the `/` menu (0.426): main/runtime-commands.ts. */
export const RUNTIME_COMMANDS_CHANNEL = 'runtime:commands'
export interface RuntimeCommandRow { readonly name: string; readonly description: string; readonly argumentHint: string }
export type RuntimeCommandsResponse = Readonly<Partial<Record<'claude' | 'opencode' | 'codex', readonly RuntimeCommandRow[]>>>
export type WorkspacePageResponse = { readonly ok: true; readonly url: string } | { readonly ok: false; readonly message: string }

/**
 * An attached image, as a `data:` URL the renderer can put in an `<img>`.
 *
 * A refusal is not an error state worth drawing: the file row is already
 * there and already says the file's name, so a preview that cannot be built
 * simply is not drawn. `message` exists for a log, not for a card.
 */
export type WorkspaceImageResponse =
  | { readonly ok: true; readonly dataUrl: string; readonly path: string }
  | { readonly ok: false; readonly message: string }

/** What came of a reveal. A refusal names why, in words a card can show. */
export type RevealFileResponse =
  | { readonly ok: true }
  | { readonly ok: false; readonly message: string }
export const OPEN_LINK_CHANNEL = 'shell:open-link'
/**
 * WHAT A REPLY ON ANOTHER RUNTIME WOULD CARRY, before it is sent (0.517).
 * Product ideas, round four: the handoff brief is composed from sections and
 * drops whole ones to fit, and nothing showed that before the send. Composed
 * exactly as the send composes it, with nothing written: no checkpoint, no
 * task file.
 */
export const HANDOFF_PREVIEW_CHANNEL = 'mission:handoff-preview'
export interface HandoffPreviewRequest {
  readonly followUpOf: string
  readonly runtime: MissionRuntimeId
  readonly prompt: string
  /** Sections the person has chosen to leave out (0.527): 'earlier', 'settled' or 'summary'. */
  readonly leaveOut?: readonly string[]
}
export type HandoffPreview =
  /** The reply stays on the same runtime: there is no brief to show. */
  | { readonly kind: 'same' }
  | {
      readonly kind: 'switch'
      readonly fromRuntime: string
      /** Sections carried, in reading order: task, earlier, unsettled, settled, summary. */
      readonly kept: readonly string[]
      /** Sections left out to fit, in the order they were given up. */
      readonly omitted: readonly string[]
      /** Actions that started and never reported back. */
      readonly unsettledCount: number
      /** The task goes as a file because it is long (0.513). */
      readonly taskByFile: boolean
      readonly taskClipped: boolean
      /** Sections the person chose to leave out (0.527), in reading order. */
      readonly leftOutByYou?: readonly string[]
    }
  /** The send would be refused, and why. */
  | { readonly kind: 'refused'; readonly message: string }
/** A newer Locust for this Mac (0.515, main/mac-release.ts): its version and disk image, or nothing. */
export const MAC_RELEASE_CHANNEL = 'updates:mac-release'
export interface MacReleaseAnswer {
  readonly version: string
  readonly url: string
}
/**
 * What came of opening a link. A refusal names why, the same as a reveal.
 *
 * The host opens only the addresses on its own list; a URL that is not one
 * of them is refused rather than opened, which is what keeps `openExternal`
 * from being a hole through the packaged build's egress rules.
 */
export type OpenLinkResponse =
  | { readonly ok: true }
  | { readonly ok: false; readonly message: string }
export const ROOM_LIST_CHANNEL = 'rooms:list'
export const ROOM_CREATE_CHANNEL = 'rooms:create'
export const ROOM_REMOVE_CHANNEL = 'rooms:remove'
/** Name a room that was made from an ask, and so began as `Untitled room`. */
export const ROOM_RENAME_CHANNEL = 'rooms:rename'
export const ROOM_POST_CHANNEL = 'rooms:post'
/** Compare (0.441, shared/compare.ts): one ask to two or three models, side by side. */
export const COMPARE_START_CHANNEL = 'compare:start'
export const COMPARE_ASK_CHANNEL = 'compare:ask'
export const COMPARE_KEEP_CHANNEL = 'compare:keep'
/** Try one column's newest ask again, on the same model (0.444). */
export const COMPARE_RETRY_CHANNEL = 'compare:retry'
/** What each column of a comparison that edits has changed so far (0.445). */
export const COMPARE_CHANGES_CHANNEL = 'compare:changes'
/** Why a comparison in this folder cannot change files, or nothing when it can (0.457). */
export const COMPARE_CHANGES_REFUSAL_CHANNEL = 'compare:changes-refusal'
/** Whether a comparison that edits would work in the folder itself here: not a git project, and too big to copy (0.675). */
export const COMPARE_IN_PLACE_CHANNEL = 'compare:in-place'
export type CompareChangesResponse =
  | { readonly ok: true; readonly data: { readonly columns: Partial<Record<CompareSlotId, { readonly files: number; readonly added?: number; readonly removed?: number }>> } }
  | { readonly ok: false; readonly error: { readonly code: 'COMPARE_REFUSED'; readonly message: string } }
export const COMPARE_LIST_CHANNEL = 'compare:list'
/** Ask a judge for its view of the answers (0.520, main/compare-judge.ts). */
export const COMPARE_JUDGE_CHANNEL = 'compare:judge'
/** Put a comparison's question to one more model, as a new column (0.523). */
export const COMPARE_ADD_MODEL_CHANNEL = 'compare:add-model'
/** Your own benchmark (0.763): a comparison run again, its runs stopped, and where every batch is. */
export const COMPARE_RERUN_CHANNEL = 'compare:rerun'
export const COMPARE_RERUN_CHECKED_CHANNEL = 'compare:rerun-checked'
export const COMPARE_RERUN_STOP_CHANNEL = 'compare:rerun-stop'
export const COMPARE_RERUNS_CHANNEL = 'compare:reruns'
/** Draft the points a judge scores (0.766): one short read-only run on the model asked. */
export const COMPARE_DRAFT_POINTS_CHANNEL = 'compare:draft-points'
export type CompareDraftPointsResponse = { readonly ok: true; readonly points: readonly string[] } | { readonly ok: false; readonly message: string }
export interface CompareAddModelRequest {
  readonly compareId: string
  readonly route: { readonly runtime: MissionRuntimeId; readonly model: string; readonly effort?: string; readonly label?: string }
}
export interface CompareJudgeRequest {
  readonly compareId: string
  readonly route: { readonly runtime: MissionRuntimeId; readonly model: string; readonly effort?: string; readonly label?: string }
  /** What the person says a good answer does; optional, and needed for three judges. */
  readonly criteria?: string
  /** Two more judges, voting with the first (0.765): a point counts when two of the three agree. */
  readonly panel?: readonly { readonly runtime: MissionRuntimeId; readonly model: string; readonly effort?: string; readonly label?: string }[]
}
export interface CompareStartRequest {
  /** Whose conversation; absent to compare with nobody. */
  readonly teammateId?: string
  readonly prompt: string
  readonly routes: readonly { readonly runtime: MissionRuntimeId; readonly model: string; readonly account?: string; readonly effort?: string; readonly label?: string; readonly mode?: 'auto'; readonly instructions?: string }[]
  /** Each model changes its own copy of the project (0.445); absent, they only answer. */
  readonly changes?: boolean
  /** The names hidden until one is kept (0.449). */
  readonly blind?: boolean
}
/** A comparison set running again (0.763), or why it was not. */
export type CompareRerunResponse = { readonly ok: true; readonly compare: PublicCompare } | { readonly ok: false; readonly message: string }
export type CompareResponse =
  | { readonly ok: true; readonly data: { readonly compare: PublicCompare; readonly refused: readonly { readonly slot: CompareSlotId; readonly message: string }[] } }
  | { readonly ok: false; readonly error: { readonly code: 'COMPARE_REFUSED'; readonly message: string } }
export type CompareListResponse =
  | { readonly ok: true; readonly data: { readonly compares: readonly PublicCompare[] } }
  | { readonly ok: false; readonly error: { readonly code: 'COMPARE_REFUSED'; readonly message: string } }
/** Tag teammates from any conversation (0.438, shared/tagging.ts). */
export const TEAMMATES_TAG_CHANNEL = 'teammates:tag'
export interface TagTeammatesRequest {
  readonly teammateIds: readonly string[]
  /** What the person sent, attachments line included. */
  readonly message: string
  /** The conversation it was sent in: its teammate, and its newest turn for context. */
  readonly fromTeammateId?: string
  readonly fromMissionId?: string
}
export type TagTeammatesResponse =
  | { readonly ok: true; readonly started: readonly string[]; readonly refused: readonly { readonly name: string; readonly message: string }[] }
  | { readonly ok: false; readonly message: string }
export const ROOM_TASK_CHANNEL = 'rooms:task'
export const MEMORY_LIST_CHANNEL = 'memory:list'
export const MEMORY_ADD_CHANNEL = 'memory:add'
export const MEMORY_UPDATE_CHANNEL = 'memory:update'
export const MEMORY_REMOVE_CHANNEL = 'memory:remove'
export const MEMORY_CLEAR_CHANNEL = 'memory:clear'
export const MEMORY_RESTORE_CHANNEL = 'memory:restore'
export const SKILL_LIBRARY_PREVIEW_CHANNEL = 'skill-library:preview'
export const SKILL_LIBRARY_INSTALL_CHANNEL = 'skill-library:install'
export const SKILL_LIBRARY_LIST_CHANNEL = 'skill-library:list'
export const SKILL_LIBRARY_REMOVE_CHANNEL = 'skill-library:remove'

/**
 * SKILLS FROM A GITHUB REPOSITORY (0.710, skill-library.ts). A person names a
 * public repository; Locust lists the skill folders in it (a folder holding a
 * SKILL.md) with every file each carries, the files a run could execute
 * marked; the person keeps the ones they pick, at the one commit the list was
 * read from. Nothing changes until they look again and keep again.
 */
export interface SkillLibraryFile {
  /** Inside the skill's folder: `scripts/fill.py`. */
  readonly path: string
  readonly bytes: number
  /** A script or program: an extension that runs, or a file GitHub records as executable. */
  readonly runs: boolean
}

export interface SkillLibrarySkill {
  /** The folder's name, which is the skill's: a run calls it `library:<name>`. */
  readonly name: string
  /** Where it sits in the repository: `skills/pdf`. */
  readonly folder: string
  /** From the SKILL.md's `description:`, when it has one. */
  readonly description?: string
  readonly bytes: number
  readonly files: readonly SkillLibraryFile[]
  /** Why it cannot be kept, when it cannot: too large, or the name is kept from another repository. */
  readonly held?: string
  /** The commit this repository's copy of it was kept at, when one is kept. */
  readonly keptAt?: string
}

export interface SkillLibraryPreview {
  /** `owner/repo`. */
  readonly source: string
  /** The branch, tag or commit the link named, or the repository's default branch. */
  readonly ref: string
  /** The commit read: what keeping installs, whatever the branch does after. */
  readonly sha: string
  readonly skills: readonly SkillLibrarySkill[]
  /** True when GitHub listed only part of a very large repository. */
  readonly partial: boolean
  /** The folder inside the repository the link named, when it named one: Keep takes the skills from there. */
  readonly path?: string
}

export type SkillLibraryPreviewResponse =
  | { readonly ok: true; readonly data: SkillLibraryPreview }
  | { readonly ok: false; readonly error: { readonly code: 'SKILL_LIBRARY_REJECTED'; readonly message: string } }

export interface SkillLibraryInstallRequest {
  readonly source: string
  readonly ref: string
  readonly sha: string
  readonly names: readonly string[]
  /** The previewed folder (SkillLibraryPreview.path). */
  readonly path?: string
}

export interface SkillLibraryKept {
  readonly name: string
  readonly files: number
  readonly bytes: number
  /** The files in it that run, by path inside the skill. */
  readonly runs: readonly string[]
}

export interface SkillLibrarySource {
  readonly source: string
  readonly ref: string
  readonly sha: string
  readonly keptAt: string
  readonly skills: readonly SkillLibraryKept[]
}

export type SkillLibraryListResponse =
  | { readonly ok: true; readonly data: { readonly sources: readonly SkillLibrarySource[] } }
  | { readonly ok: false; readonly error: { readonly code: 'SKILL_LIBRARY_REJECTED'; readonly message: string } }

export const RUNTIME_SETUP_CHANNEL = 'runtime:setup'
/** Every connector the person's Claude Code reports, and which each teammate may use. */
export const CONNECTOR_LIST_CHANNEL = 'connectors:list'
export const TEAMMATE_CONNECTORS_CHANNEL = 'teammates:connectors'
/** Pick, or clear, the folder one teammate works in. The host names the path. */
export const TEAMMATE_FOLDER_CHANNEL = 'teammates:folder'
export const WORKTREE_LIST_CHANNEL = 'worktrees:list'
export const WORKTREE_REMOVE_CHANNEL = 'worktrees:remove'
/** Review changes (0.439): a teammate's own branch against where it left the person's, and one turn of it. */
export const WORKTREE_REVIEW_CHANNEL = 'worktrees:review'
export const WORKTREE_TURN_DIFF_CHANNEL = 'worktrees:turn-diff'

export interface PublicBranchTurn {
  readonly sha: string
  readonly subject: string
  readonly at: string
  readonly files: readonly string[]
  /** It finished a merge with the person's branch (0.440): git names no files of its own for it. */
  readonly merge?: true
}

export interface PublicBranchReview {
  readonly branch: string
  /** The person's branch it is measured against; undefined when their checkout is detached. */
  readonly against: string | undefined
  readonly base: string
  /** Oldest first: one per turn that changed something. */
  readonly turns: readonly PublicBranchTurn[]
  /** The whole change as a unified diff; undefined when too large to show at once. */
  readonly diff: string | undefined
  /** Changes in the tree no turn has saved yet. */
  readonly uncommitted: readonly string[]
}

export type BranchReviewResponse =
  | { readonly ok: true; readonly data: PublicBranchReview }
  | { readonly ok: false; readonly error: { readonly code: 'REVIEW_UNAVAILABLE'; readonly message: string } }

/** Land it (0.440): preview, land, and begin resolving a conflict on the teammate's branch. */
export const WORKTREE_LAND_PREVIEW_CHANNEL = 'worktrees:land-preview'
export const WORKTREE_LAND_CHANNEL = 'worktrees:land'
export const WORKTREE_RESOLVE_CHANNEL = 'worktrees:resolve'

/** Why a branch cannot land now; `busy` is the host's own (a run is live for the teammate). */
export type PublicLandBlock =
  | { readonly kind: 'nothing' | 'detached' | 'merging' | 'busy' | 'markers-unchecked' }
  | { readonly kind: 'old-git'; readonly version: string | undefined }
  | { readonly kind: 'unsaved' | 'markers' | 'your-changes' | 'conflicts'; readonly files: readonly string[] }

export interface PublicLandPreview {
  readonly branch: string
  readonly onto: string | undefined
  readonly files: readonly string[]
  readonly draft: string
  readonly block: PublicLandBlock | undefined
}

export type LandPreviewResponse =
  | { readonly ok: true; readonly data: PublicLandPreview }
  | { readonly ok: false; readonly error: { readonly code: 'REVIEW_UNAVAILABLE'; readonly message: string } }

export type LandResponse =
  | { readonly ok: true; readonly data:
      | { readonly kind: 'landed'; readonly sha: string; readonly onto: string; readonly files: readonly string[]; readonly branchReset: boolean }
      | { readonly kind: 'blocked'; readonly block: PublicLandBlock }
      | { readonly kind: 'refused'; readonly message: string } }
  | { readonly ok: false; readonly error: { readonly code: 'REVIEW_UNAVAILABLE'; readonly message: string } }

export type ResolveResponse =
  | { readonly ok: true; readonly data: { readonly onto: string; readonly files: readonly string[] } }
  | { readonly ok: false; readonly error: { readonly code: 'REVIEW_UNAVAILABLE'; readonly message: string } }

export type TurnDiffResponse =
  | { readonly ok: true; readonly data: { readonly diff: string } }
  | { readonly ok: false; readonly error: { readonly code: 'REVIEW_UNAVAILABLE'; readonly message: string } }
export const MISSION_APPROVAL_CHANNEL = 'mission-approval:request'
export const MISSION_APPROVAL_DECIDE_CHANNEL = 'mission-approval:decide'
/**
 * SAVED APPROVAL RULES (0.521, shared/approval-rules.ts). The host answers a
 * card a rule covers and says which rule did; these list the rules, remove
 * one, and make one from a card on screen -- built by the HOST from the
 * request it holds, so the window can never ask for more than the card showed.
 */
export const APPROVAL_RULES_LIST_CHANNEL = 'approval-rules:list'
export const APPROVAL_RULES_REMOVE_CHANNEL = 'approval-rules:remove'
/** Every saved rule at once (0.587): Settings > Teammates, "Remove all rules". */
export const APPROVAL_RULES_REMOVE_ALL_CHANNEL = 'approval-rules:remove-all'
export const APPROVAL_RULE_FROM_CARD_CHANNEL = 'approval-rules:from-card'
export interface PublicApprovalRule {
  readonly ruleId: string
  readonly effect: 'allow' | 'deny'
  readonly kind: 'command' | 'edit' | 'read' | 'connector'
  readonly pattern: string
  readonly teammateId?: string
  readonly folder?: string
  readonly createdAt: string
  readonly uses?: number
  readonly lastUsedAt?: string
}
export type ApprovalRulesResponse =
  | { readonly ok: true; readonly rules: readonly PublicApprovalRule[] }
  | { readonly ok: false; readonly message: string }
export interface ApprovalRuleFromCardRequest {
  readonly approvalId: string
  /** allow: "Yes, and don't ask again"; deny: "No, and never". */
  readonly effect: 'allow' | 'deny'
  /** With a deny, the reason the teammate reads. */
  readonly reason?: string
}
/** An approval answered somewhere else -- a question answered in Antigravity's own window. */
export const MISSION_APPROVAL_WITHDRAWN_CHANNEL = 'mission-approval:withdrawn'

export type LocalRuntimeId = 'codex' | 'claude' | 'cursor' | 'gemini' | 'opencode' | 'copilot' | 'antigravity' | 'muse' | 'omniroute'

export type TeammateHue = 'lime' | 'blue' | 'violet' | 'clay' | 'teal' | 'butter' | 'rose' | 'slate' | 'pearl' | 'indigo' | 'coral'

export type TeammateRole =
  | 'Code & Migrations'
  | 'Research & Briefs'
  | 'Ops & Scheduling'
  | 'Docs & QA'
  | 'Data & Reporting'
  | 'Chief of Staff'
  | 'Custom'

/**
 * A teammate is local identity and routing defaults. It owns no process and
 * grants no capability -- the roster is real without a multi-agent runtime
 * behind it.
 */
export interface PublicTeammate {
  readonly teammateId: string
  readonly name: string
  readonly hue: TeammateHue
  readonly role: TeammateRole
  /**
   * What a Custom teammate actually does, in the person's own words: the
   * title shown beside their name and the role their runtime is told. The
   * built-in roles carry their own name and need none; a Custom teammate
   * with no title is shown as, and briefed as, "Custom".
   */
  readonly roleTitle?: string
  /**
   * What this teammate offers as a first message, on their empty
   * conversation (0.359). Set by the team template they came from: Sable,
   * the analyst in Research & money, offered "Summarize what this codebase
   * is for" because her ROLE is Research & Briefs, and so did every other
   * template teammate outside Build software (the first-session drive,
   * packaged 0.358). Absent, the role's own are offered.
   */
  readonly starters?: readonly string[]
  /**
   * Works on its own branch: missions run in this teammate's own worktree of
   * the folder's repository (`.locust/worktrees/<id>`, branch `locust/<name>`),
   * so two teammates editing one repository do not collide. Off by default.
   */
  readonly worktree?: boolean
  /**
   * The folder THIS teammate works in, when it is not the project folder.
   *
   * Colin, 2026-09-09: "it should only change the folder for that chat/
   * teammate not the entire app." Switching the project folder reopens
   * Locust, because the ledger, the memory store and the worktrees are all
   * scoped by it; a teammate's own folder is a narrower thing that needs
   * none of that, because it only decides where that teammate's runs stand.
   *
   * It is also the only way to reach a local MCP server, which Claude Code
   * registers under a PROJECT KEY in `~/.claude.json`: a server declared for
   * `C:/Users/<home>/claude` exists in that folder and nowhere else, so a
   * teammate that needs it has to be standing there.
   *
   * Written by the HOST from a folder dialog, never named by the renderer.
   * History stays filed under the project folder either way.
   */
  readonly folder?: string
  /**
   * The connectors THIS teammate may use, by the name the CLI prints.
   *
   * Absent means every connector the person's own Claude Code can reach --
   * Colin's ruling, and the ordinary state. Present, it NARROWS: only these
   * are allowed without asking, and a call to any other stops the run and
   * asks through the permission host. Never a grant of something the person
   * does not have: a name that is not on the machine is simply a rule for a
   * server that never appears.
   *
   * Written from the teammate dialog, from a list the host read off
   * `claude mcp list`; never typed.
   */
  readonly connectors?: readonly string[]
  /**
   * The face, persisted with the record. Seeded from the immutable id when a
   * teammate is created without one, so a rename never changes it.
   */
  readonly avatar: AvatarSpec
  readonly createdAt: string
  /**
   * The route this teammate last ran on, recorded by the host from the
   * missions a person started for them. A teammate replying on their own
   * runs HERE -- their own runtime, model and mode -- never on whoever
   * wrote to them. People will pit one model against another on purpose,
   * and that only means anything if each side stays itself.
   */
  readonly route?: TeammateRoute
  /**
   * The latest turn of this teammate's HUB: the one conversation their
   * replies to other teammates land in.
   *
   * Colin, 2026-09-21: "its kind of messy that each time a teammate messages
   * another that it spawns a new chat in ungrouped ... each teammate has
   * their own isolated chat where the replies go to on the sidebar." A
   * reply inside an exchange continues the recipient's own mission in that
   * exchange; a reply that has no such predecessor used to start a brand-new
   * root, and every one of those was another Ungrouped row. Now it continues
   * the hub, and the hub is the row a face opens.
   *
   * Written by the host from the relay, never by the renderer. Absent until
   * the teammate first replies on their own.
   */
  readonly hubMissionId?: string
  /**
   * The most, in dollars, this teammate may spend in a calendar month.
   *
   * Paperclip's idea, the one worth taking (2026-09-26): a teammate that
   * replies and runs routines on its own can spend while nobody watches. The
   * host checks it before EVERY start -- a message, a relayed reply, a
   * routine step, a room post -- and refuses once the month's priced runs
   * reach it. Only dollars count; a plan's runs and Copilot's premium
   * requests are not dollars (shared/spend.ts). Absent means no limit.
   */
  readonly monthlyLimitUsd?: number
  /**
   * THIS TEAMMATE'S OWN STANDING INSTRUCTIONS (0.706, from the Paperclip
   * scrub: "agents keep their own files"). Every turn this teammate takes is
   * briefed with them, after a group's and before memory. At most
   * MAX_TEAMMATE_INSTRUCTIONS characters; absent means none.
   */
  readonly instructions?: string
  /** The voice it speaks in when the person presses Talk in its conversation (0.746): one of ChatGPT's nine; absent, Talk's own. */
  readonly voice?: string
}

/** The longest a teammate's own instructions may be, as a group's (0.706). */
export const MAX_TEAMMATE_INSTRUCTIONS = 4_000

export interface TeammateRoute {
  readonly account?: string
  readonly runtime: MissionRuntimeId
  /** A model id from the catalog, or `account-default`. */
  readonly model: string
  readonly mode: MissionMode
  /**
   * The reasoning effort this route runs at, when its model reports levels.
   *
   * A routine replays the turns you typed, and how hard the model was asked to
   * think is part of how it ran -- one saved while set to `high` that replays
   * at the runtime's default is not the same routine. Absent on every runtime
   * whose models report no levels, which is also every runtime whose command
   * builder refuses an effort outright.
   */
  readonly effort?: string
}

/**
 * A workroom message as the renderer sees it: who said it to whom, and the
 * text. `direction` is relative to the mission being shown -- `received` was
 * quoted into that mission's prompt, `posted` came out of its work. `text` is
 * null when the ledger still points at a message the workroom no longer holds,
 * which is said rather than hidden.
 */
export interface PublicPeerMessage {
  readonly messageId: string
  readonly direction: 'received' | 'posted'
  /**
   * `missionId` is the sender's own mission -- the work that produced this
   * message. The store has always carried it (`WorkroomSender.missionId`);
   * the public shape dropped it, so the receiving side could name the
   * teammate and not reach them. Carried now, because a message is
   * attributable to exactly one piece of work and the reader should be able
   * to go there.
   */
  readonly from: { readonly teammateId: string; readonly name: string; readonly missionId?: string }
  readonly to: { readonly teammateId: string; readonly name: string }
  readonly text: string | null
  readonly at: string
}

export interface TeammateCreateRequest {
  readonly name: string
  readonly hue: TeammateHue
  readonly role: TeammateRole
  readonly roleTitle?: string
  readonly worktree?: boolean
  /** The look chosen in the dialog; omitted, the store seeds one from the new id. */
  readonly avatar?: AvatarSpec
  /** The model picked on the dialog's Model row; omitted, their first mission's is kept. */
  readonly route?: TeammateRoute
  /** Dollars a month; omitted, no limit. */
  readonly monthlyLimitUsd?: number
  /** Its own standing instructions (0.706); omitted or empty, none. */
  readonly instructions?: string
  /** Its voice in a call (0.746); omitted, Talk's own. */
  readonly voice?: string
  /** A template's first messages for this teammate; omitted, the role's are offered. */
  readonly starters?: readonly string[]
}

/**
 * Everything about a teammate a person may change. The id is what they are
 * and stays; missions filed under them stay filed.
 */
export interface TeammateUpdateRequest {
  readonly teammateId: string
  readonly name: string
  readonly hue: TeammateHue
  readonly role: TeammateRole
  readonly roleTitle?: string
  readonly worktree?: boolean
  readonly avatar: AvatarSpec
  /**
   * The model picked on the dialog's Model row (0.311). Colin, 2026-09-24:
   * "do we have the ability to switch a teammates model? like not when
   * youre in the chat but the actual designated teammate". Omitted, the
   * route stays what it was.
   */
  readonly route?: TeammateRoute
  /**
   * Dollars a month. `null` removes the limit; OMITTED keeps whatever it is,
   * so no other edit can quietly lift a limit the person set.
   */
  readonly monthlyLimitUsd?: number | null
  /** Its own standing instructions (0.706): text sets them, `null` or empty removes them, OMITTED keeps them. */
  readonly instructions?: string | null
  /** Its voice in a call (0.746): one of ChatGPT's nine sets it, `null` removes it, OMITTED keeps it. */
  readonly voice?: string | null
}

export type TeammateListResponse =
  | {
      readonly ok: true
      readonly data: {
        readonly teammates: readonly PublicTeammate[]
        readonly missionOwners: Readonly<Record<string, string>>
        /** Names people typed for conversations, by mission id. */
        readonly missionTitles: Readonly<Record<string, string>>
        /** Conversations pinned to the sidebar's top, newest pin first (0.729). */
        readonly missionPins?: readonly string[]
        /** Conversations settled or snoozed out of the sidebar: when, and until when (0.730). */
        readonly missionSettled?: Readonly<Record<string, { readonly at: string; readonly until?: string }>>
      }
    }
  | { readonly ok: false; readonly error: { readonly code: 'TEAMMATES_UNAVAILABLE'; readonly message: string } }

/** A pet on this computer, wearable as a teammate's face (0.563). */
export interface PublicPet {
  readonly source: PetSource
  readonly id: string
  readonly displayName: string
  readonly description: string
  /** 9 (a version 1 sheet) or 11 (version 2: a resting pose, and a head that turns). */
  readonly rows: PetRows
}

/** Every pet refusal is said in words a person can act on. */
export interface PetFailure {
  readonly ok: false
  readonly error: { readonly code: 'PETS_UNAVAILABLE' | 'REFUSED' | 'WORN'; readonly message: string }
}

/** `removed` (0.569): the picks the person took out of the picker. */
export type PetListResponse = { readonly ok: true; readonly data: { readonly pets: readonly PublicPet[]; readonly removed?: readonly string[] } } | PetFailure
export type PetSheetResponse = { readonly ok: true; readonly data: { readonly bytes: Uint8Array; readonly rows: PetRows } } | PetFailure
export type PetThumbnailResponse = { readonly ok: true; readonly data: { readonly dataUrl: string } } | PetFailure
export type PetAddResponse = { readonly ok: true; readonly data: { readonly pet: PublicPet } } | PetFailure
/** `keptFor` (0.569): who wears it, so its files stayed for their face. */
export type PetRemoveResponse = { readonly ok: true; readonly data?: { readonly keptFor: readonly string[] } } | PetFailure

/**
 * Each teammate's money this calendar month, by id. A teammate with none is
 * absent -- no money, not "$0.00" -- so a card that reads it cannot turn a
 * runtime that reported nothing into "free".
 */
export type TeammateSpendResponse =
  | { readonly ok: true; readonly data: { readonly byTeammate: Readonly<Record<string, Spend>> } }
  | { readonly ok: false; readonly error: { readonly code: 'TEAMMATES_UNAVAILABLE'; readonly message: string } }

export type TeammateMutationResponse =
  | { readonly ok: true; readonly data: { readonly teammate?: PublicTeammate } }
  | { readonly ok: false; readonly error: { readonly code: 'TEAMMATE_REJECTED'; readonly message: string } }

/** One MCP server the CLI reported, as the person would recognise it. */
export interface PublicConnector {
  readonly name: string
  /** A URL for a remote connector, a command for a local one. */
  readonly location: string
  /**
   * `needs-auth` is a real connector the person has not finished signing
   * into; `failed`, Claude Code could not start or reach it; `timed-out`, the
   * last check did not answer in 20 seconds; `unreadable`, words Locust does
   * not know (in `said`) -- W8, 0.567.
   */
  readonly status: 'connected' | 'needs-auth' | 'failed' | 'timed-out' | 'unreadable'
  readonly said?: string
  /** How long the last check took, in ms. */
  readonly checkedInMs?: number
  /** When it was last seen connected, since Locust started. */
  readonly lastConnectedAt?: string
}

export type ConnectorListResponse =
  | { readonly ok: true; readonly data: { readonly connectors: readonly PublicConnector[] } }
  | { readonly ok: false; readonly error: { readonly code: 'CONNECTORS_UNAVAILABLE'; readonly message: string } }

/**
 * The answer to a folder request for one teammate.
 *
 * `CANCELLED` is the ordinary outcome -- the person closed the dialog -- and
 * is never drawn as trouble.
 */
export type TeammateFolderResponse =
  | { readonly ok: true; readonly data: { readonly teammate: PublicTeammate } }
  | {
      readonly ok: false
      readonly error: {
        readonly code: 'CANCELLED' | 'REJECTED'
        readonly message: string
      }
    }

/**
 * Who takes one step of a routine (0.435): a HAND-OFF CHAIN. From OpenRig's
 * "conveyor" (mvschwarz/openrig, Apache-2.0, read 2026-09-27 at Colin's
 * "anything worth yoinking"): steps go to different teammates in order, each
 * given the step before's answer, and a CHECKER step must approve before the
 * run counts as done. An empty entry is the routine's own teammate.
 */
export interface RoutineHandOff {
  /** The teammate who takes this step; absent, the routine's own. */
  readonly teammateId?: string
  /** This step checks the work: the run is done only if it approves. */
  readonly check?: true
}

/** A scheduled time that passed while Locust was closed. */
export interface RoutineHistoryEntry {
  readonly kind: 'missed'
  /** Calls lost to the relay's week or storage cap, rather than a clock slot. */
  readonly hookCalls?: number
  readonly dueAt: string
  readonly recordedAt: string
}

/**
 * A routine: a conversation a person saved as steps a teammate can replay.
 * The teammate cannot watch a person work outside the app; what it can learn
 * from is work it did WITH them, which every mission already records. So a
 * routine is the words typed on each turn of one conversation, in order,
 * editable, tied to the teammate and the route it was learned on.
 */
export interface PublicRoutine {
  readonly routineId: string
  readonly name: string
  readonly teammateId: string
  /** The route the routine was learned on and replays on: a read-only routine stays read-only. */
  readonly route: TeammateRoute
  /** What the person typed on each turn, in order. Corrections are edits here. */
  readonly steps: readonly string[]
  /** The missions the steps were taken from, oldest first. A routine can always show where it came from. */
  readonly learnedFrom: readonly string[]
  readonly createdAt: string
  readonly runs: number
  readonly lastRunAt?: string
  /**
   * When it runs on its own: every N hours from the last run, or daily at
   * a wall-clock time -- local, and only while the app is open. Absent
   * means what every routine was before: it runs when a person presses Run.
   */
  readonly schedule?: RoutineSchedule
  readonly execution?: import('./routine-recovery.js').RoutineExecution
  /**
   * The folder it was made in, as a workspace id. A schedule runs it only
   * there (M15); Run still runs it wherever a person presses it. Absent on a
   * routine saved before this was recorded.
   */
  readonly workspaceId?: string
  /** Who takes each step, in step order (0.435). Absent: every step is the routine's own teammate's. */
  readonly handOffs?: readonly RoutineHandOff[]
  /**
   * It works in a copy of the folder (0.533): what a run changes waits for the
   * person to Keep or Discard, and nothing lands in the folder before that.
   */
  readonly inCopy?: true
  /** A finished run's changes, waiting in its copy for Keep or Discard (0.533). */
  readonly staged?: RoutineStaged
  /**
   * A standing goal (0.534): when its steps are done, the folder's check runs,
   * and while it fails the teammate is asked to fix it, up to `tries` times.
   */
  readonly untilCheck?: RoutineGoal
  /**
   * The last run ended with its check still failing (0.536), in the words its
   * conversation said it. Cleared by the next run. Sol, 0.535: the card said
   * only "run 1 time" and offered Run again, as if it had gone fine.
   */
  readonly lastFailed?: string
  /**
   * The scheduled time that passed while Locust was closed. The card says
   * "Missed, 08:00 today" until a real run clears it. Not a completed run.
   */
  readonly missedAt?: string
  /**
   * PAUSED (0.705, from Paperclip's routine management): its schedule or
   * folder watch does not start it, and nothing is recorded as missed; Run
   * still runs it. Absent means it goes on its own, as every routine did.
   */
  readonly paused?: true
  /**
   * When it was last resumed. Its clock counts from here or its last run,
   * whichever is later, so resuming never fires a run that is long overdue.
   */
  readonly resumedAt?: string
  /**
   * Misses, oldest first, capped. A real run does not erase them: the miss
   * stays in the history after the card has moved on.
   */
  readonly history?: readonly RoutineHistoryEntry[]
  /**
   * What the routine asks for when it runs (W7): at most 12, each answering a
   * `{{key}}` in the steps. Absent: it asks for nothing, as every routine did.
   */
  readonly inputs?: readonly RoutineInput[]
}

/** Keep going until the folder's check passes, at most this many fixes (0.534). */
export interface RoutineGoal {
  readonly tries: number
}
export const MAX_GOAL_TRIES = 5

/** What a routine's last run changed in its copy, waiting for the person (0.533). */
export interface RoutineStaged {
  readonly attemptId: string
  readonly finishedAt: string
  /** The folder it was copied from, which Keep writes into. */
  readonly folder: string
  /** Changed or new files, then deleted ones, by their path in the folder. */
  readonly changed: readonly string[]
  readonly deleted: readonly string[]
}

export interface RoutineCreateRequest {
  readonly name: string
  readonly teammateId: string
  readonly route: TeammateRoute
  readonly steps: readonly string[]
  readonly learnedFrom: readonly string[]
  readonly schedule?: RoutineSchedule
  readonly handOffs?: readonly RoutineHandOff[]
  /** Works in a copy, kept or discarded by the person (0.533). */
  readonly inCopy?: boolean
  /** Keep going until the check passes (0.534). */
  readonly untilCheck?: RoutineGoal
  /** What it asks for when it runs (W7). */
  readonly inputs?: readonly RoutineInput[]
}

export interface RoutineUpdateRequest {
  readonly routineId: string
  readonly name: string
  readonly steps: readonly string[]
  /** One per step; absent keeps them only while the steps still line up. */
  readonly handOffs?: readonly RoutineHandOff[]
  /** `null` clears a schedule; absent leaves it as it was. */
  readonly schedule?: RoutineSchedule | null
  /** The mode its runs use (0.530: "Only read" or "Change files" in the dialog); absent keeps it. */
  readonly mode?: MissionMode
  /** Works in a copy (0.533); absent keeps it as it was. */
  readonly inCopy?: boolean
  /** Keep going until the check passes (0.534); `null` stops after its steps; absent keeps it. */
  readonly untilCheck?: RoutineGoal | null
  /** What it asks for (W7); `null` asks for nothing; absent keeps what it had. */
  readonly inputs?: readonly RoutineInput[] | null
}

export type RoutineListResponse =
  | { readonly ok: true; readonly data: { readonly routines: readonly PublicRoutine[] } }
  | { readonly ok: false; readonly error: { readonly code: 'ROUTINES_UNAVAILABLE'; readonly message: string } }

export type RoutineMutationResponse =
  | { readonly ok: true; readonly data: { readonly routine?: PublicRoutine } }
  | { readonly ok: false; readonly error: { readonly code: 'ROUTINE_REJECTED'; readonly message: string } }

/**
 * A room: a named set of teammates a person can write to at once, and the
 * posts they made. A post starts one ordinary mission per teammate, on that
 * teammate's own route; the room remembers which, and the thread a person
 * watches is read from those missions' records.
 */
export interface RoomPost {
  readonly postId: string
  readonly text: string
  readonly at: string
  /** The mission each teammate answered in, by teammate id. A teammate whose run could not start is absent. */
  readonly missions: Readonly<Record<string, string>>
  /**
   * Who is still waiting for a slot, in the order they will get one.
   *
   * A post asks everyone in the room, but only `MAX_LIVE_MISSIONS` can run
   * at once and a teammate already working cannot take a second mission. A
   * member in that position is not refused -- waiting fixes it -- so they
   * wait here and start when a slot frees.
   *
   * Before this existed they were simply never asked, and the room said so
   * and moved on. That cost was permanent and it was paid by the RECORD:
   * a teammate who never started left no mission at all, so the next day
   * nothing showed they had been asked.
   *
   * A name leaves this list the moment its mission starts, and appears in
   * `missions` instead. Absent on posts written before this existed.
   */
  readonly queued?: readonly string[]
  /**
   * Why a member was not asked at all, by teammate id, in the host's own
   * words. Only reasons waiting cannot fix -- gone from the roster, a
   * runtime a room cannot post to. Anything retryable queues instead.
   *
   * Recorded because the reason is a fact about the PAST. It used to live
   * only in the response to the post -- shown once in the composer note,
   * then overwritten by the next thing the room had to say -- so a reload,
   * or simply waiting for the room to finish, left the absence with no
   * explanation at all. Absent on posts written before this existed.
   */
  readonly refused?: Readonly<Record<string, string>>
  /**
   * Who the person put this post to, when they named someone (0.371).
   *
   * A post asked everyone in the room, and "Wren, say more about your second
   * point" ran every member -- each of them answering a question meant for
   * one. Open WebUI's channels and Buzz both answer only the agent that is
   * mentioned (docs/RESEARCH-2026-09-26-ROOMS-AND-PEERS.md). Recorded on the
   * post so the room can say who was asked, and so the room's history tells
   * the others it was not put to them. Absent means everyone.
   */
  readonly to?: readonly string[]
}

/**
 * A task on a room's board: a line of text, an owner, a state, and the
 * mission that last touched it (vision #2, "ownership"). Teammates move it
 * with a task block at the end of a reply; a person moves it from the room.
 */
export interface RoomTask {
  readonly taskId: string
  readonly text: string
  readonly ownerId: string | undefined
  readonly state: 'open' | 'in-hand' | 'done'
  /** The mission whose reply last moved this task, when a teammate did. */
  readonly missionId: string | undefined
  /**
   * The teammate who last handed it to its owner (A2.15): a handoff straight
   * back to them is refused, so a stage is never bounced between two members.
   * Cleared when someone claims it or a person reassigns it.
   */
  readonly handedBy?: string | undefined
  /**
   * A2.6: handed over with no message to its new owner in the same reply --
   * so the board tells them there is nothing to go on but the row itself.
   * Cleared with `handedBy`.
   */
  readonly handedWithoutNote?: boolean
  readonly at: string
}

/**
 * A named set of conversations, in this folder.
 *
 * More than a folder, by Colin's ruling of 2026-09-15: a group carries
 * standing instructions and a default route, so filing a conversation into
 * one buys something. `instructions` and `route` are part of the shape from
 * the start even while nothing writes them yet -- a group that learned about
 * them later would mean migrating a file people already had.
 *
 * The route is a SEED, not an override: it applies once, when a conversation
 * is created in the group, and the conversation owns its route from then on.
 * So changing a group's default can never reach back and silently rewrite
 * the route of work that has already run.
 */
export interface PublicGroup {
  readonly groupId: string
  readonly name: string
  readonly createdAt: string
  /** What every conversation started in this group is briefed with. */
  readonly instructions: string
  /** What a conversation started here begins on. Absent means no default. */
  readonly route?: TeammateRoute
}

export const GROUP_LIST_CHANNEL = 'groups:list'
export const GROUP_CREATE_CHANNEL = 'groups:create'
export const GROUP_RENAME_CHANNEL = 'groups:rename'
export const GROUP_REMOVE_CHANNEL = 'groups:remove'
/** Move a conversation into a group, or out of one with no group id. */
export const GROUP_ASSIGN_CHANNEL = 'groups:assign'
/** Set what every conversation in a group is briefed with. Empty clears it. */
export const GROUP_INSTRUCTIONS_CHANNEL = 'groups:instructions'
export const GROUP_ROUTE_CHANNEL = 'groups:route'

/**
 * Which group a conversation is in, and when it joined.
 *
 * The moment matters: instructions brief from joining onward and never
 * retroactively, so the thread marks the boundary rather than claiming the
 * turns above it were briefed too.
 */
export interface GroupMembership {
  readonly groupId: string
  readonly at?: string
}

/**
 * A membership that ended: the conversation left the group, was moved to
 * another, or the group was removed. The group's name and words are kept AS
 * THEY WERE, because the line in the thread ("Trading's instructions no
 * longer apply from here") has to stay true after the group is renamed,
 * edited or gone.
 */
export interface LeftMembership {
  readonly groupId: string
  readonly name: string
  readonly instructions: string
  /** When it joined, if that was recorded. */
  readonly at?: string
  /** When it left, ISO. */
  readonly until: string
}

export type GroupListResponse =
  | {
      readonly ok: true
      readonly data: {
        readonly groups: readonly PublicGroup[]
        readonly members: Readonly<Record<string, GroupMembership>>
        /** Memberships that ended, by conversation, oldest first. */
        readonly left: Readonly<Record<string, readonly LeftMembership[]>>
      }
    }
  | { readonly ok: false; readonly error: { readonly code: string; readonly message: string } }

export type GroupMutationResponse =
  | { readonly ok: true; readonly data: Record<string, never> }
  | { readonly ok: false; readonly error: { readonly code: string; readonly message: string } }

export interface PublicRoom {
  readonly roomId: string
  readonly name: string
  readonly teammateIds: readonly string[]
  readonly createdAt: string
  readonly posts: readonly RoomPost[]
  readonly tasks: readonly RoomTask[]
}

/** A person moving the board: add a task, hand it to someone, finish it, or open it again. */
export interface RoomTaskRequest {
  readonly roomId: string
  readonly op: 'add' | 'assign' | 'done' | 'reopen' | 'remove'
  readonly taskId?: string
  readonly text?: string
  /** For `assign`: a teammate in the room, or undefined to leave it with nobody. */
  readonly ownerId?: string
}

export type RoomTaskResponse =
  | { readonly ok: true; readonly data: { readonly room: PublicRoom } }
  | { readonly ok: false; readonly error: { readonly code: 'ROOM_REJECTED'; readonly message: string } }

export interface RoomCreateRequest {
  readonly name: string
  readonly teammateIds: readonly string[]
}

export interface RoomPostRequest {
  readonly roomId: string
  readonly text: string
  /**
   * The members this post is put to, by teammate id (0.371): a Reply on an
   * answer, or an @ in the room's box. Absent or empty means everyone in the
   * room, as every post was before.
   */
  readonly to?: readonly string[]
}

export type RoomListResponse =
  | { readonly ok: true; readonly data: { readonly rooms: readonly PublicRoom[] } }
  | { readonly ok: false; readonly error: { readonly code: 'ROOMS_UNAVAILABLE'; readonly message: string } }

export type RoomMutationResponse =
  | { readonly ok: true; readonly data: { readonly room?: PublicRoom } }
  | { readonly ok: false; readonly error: { readonly code: 'ROOM_REJECTED'; readonly message: string } }

/** A post, with what it started and who it could not start, in the host's words. */
export type RoomPostResponse =
  | {
      readonly ok: true
      readonly data: {
        readonly post: RoomPost
        readonly refused: readonly { readonly teammateId: string; readonly name: string; readonly message: string }[]
      }
    }
  | { readonly ok: false; readonly error: { readonly code: 'ROOM_REJECTED'; readonly message: string } }

export type RoutineRunResponse =
  | { readonly ok: true; readonly data: { readonly missionId: string; readonly runId: string } }
  | { readonly ok: false; readonly error: { readonly code: 'ROUTINE_REJECTED'; readonly message: string } }

/** A folder chosen for a routine's input; the only way a folder value gets into a run (W7). */
export type RoutineFolderResponse =
  | { readonly ok: true; readonly data: { readonly path: string } }
  | { readonly ok: false; readonly error: { readonly code: 'CANCELLED' | 'REJECTED'; readonly message: string } }

/**
 * Export a routine as a file (W7). A step that names an absolute path is not
 * written until the person says what to do with it: `input` turns each path into a
 * text input (a path can name a file too), `keep` writes it as it is.
 */
export interface RoutineExportRequest {
  readonly routineId: string
  readonly paths?: 'input' | 'keep'
}
export interface RoutineFlaggedPath {
  /** The step, counting from 1. */
  readonly step: number
  readonly path: string
}
export type RoutineExportResponse =
  | { readonly ok: true; readonly data: { readonly path?: string; readonly cancelled?: true } }
  | { readonly ok: false; readonly error: { readonly code: 'ROUTINE_REJECTED'; readonly message: string } }
  | { readonly ok: false; readonly error: { readonly code: 'ABSOLUTE_PATHS'; readonly message: string; readonly flagged: readonly RoutineFlaggedPath[] } }

/** What a routine file holds, shown before anything is created (W7). */
export interface RoutineImportPreview {
  /** Names the file the host read, for the Import that follows. */
  readonly token: string
  readonly name: string
  readonly steps: readonly string[]
  readonly inputs: readonly RoutineInput[]
  /** The role of whoever took each step where it was made; absent where it was the routine's own teammate. */
  readonly handOffRoles: readonly (string | undefined)[]
  /** Which step checks the work, one per step; a chain is a routine with one (it opens in the editor, a teammate proposed for each role). */
  readonly handOffChecks: readonly boolean[]
  /** A starter routine that changes files, as the editor should open it: changing them, in a copy. */
  readonly changesFiles?: true
  readonly runtime?: string
  /** Each connector the steps name, and whether this machine has it. */
  readonly connectors: readonly { readonly name: string; readonly present: boolean }[]
}
/** One starter routine as the list offers it (0.615). */
export interface RoutineTemplateInfo {
  /** The file's name without its suffix: what the window asks for it by. */
  readonly id: string
  readonly name: string
  /** What it does, in one line. */
  readonly summary: string
  readonly steps: number
  /** What it will ask for when it runs, by label. */
  readonly asks: readonly string[]
  /** The steps go to different teammates and the last one checks: a chain, opened in the editor. */
  readonly chain?: true
}
export type RoutineTemplatesResponse =
  | { readonly ok: true; readonly data: { readonly templates: readonly RoutineTemplateInfo[] } }
  | { readonly ok: false; readonly error: { readonly code: 'ROUTINE_REJECTED'; readonly message: string } }
export type RoutineImportPreviewResponse =
  | { readonly ok: true; readonly data: { readonly preview?: RoutineImportPreview } }
  | { readonly ok: false; readonly error: { readonly code: 'ROUTINE_REJECTED'; readonly message: string } }
export interface RoutineImportRequest {
  readonly token: string
  /** Who is given it; the routine runs on their route, read-only until its owner changes that. */
  readonly teammateId: string
  readonly route: TeammateRoute
}
export type RuntimeAuthState = 'authenticated' | 'unauthenticated' | 'unknown' | 'not-applicable'
export type RuntimeProbeStatus = 'ready' | 'not-installed' | 'auth-required' | 'offline' | 'probe-failed' | 'unsupported'

export interface PublicRuntimeStatus {
  readonly id: LocalRuntimeId
  readonly displayName: string
  readonly installed: boolean
  readonly version: string | null
  readonly auth: RuntimeAuthState
  readonly ready: boolean
  readonly status: RuntimeProbeStatus
  /** Why an installed version cannot be run yet, and what to do (0.805: OpenCode 2). Only with status 'unsupported'. */
  readonly unsupportedNote?: string
  /** Antigravity runs through Antigravity CLI rather than its app (0.541). */
  readonly throughCli?: true
  /**
   * Installed, and its check has not finished: the sweep went on without it so
   * no slow agent holds the first screen. Not signed out, not missing -- its
   * answer arrives by itself.
   */
  readonly checking?: true
  /** What an Agent Client Protocol agent said it can do, at its last run (W12, 0.566). */
  readonly agentCapabilities?: PublicAgentCapabilities
}

/** An ACP agent's own answer to `initialize`, in plain fields; shown, never acted on. */
export interface PublicAgentCapabilities {
  readonly continuesSessions: boolean
  readonly images: boolean
  readonly audio: boolean
  readonly embeddedContext: boolean
  readonly mcpOverHttp: boolean
  readonly mcpOverSse: boolean
  /** When it said so. */
  readonly toldAt: string
}

export type RuntimeDiscoveryResponse =
  | {
      readonly ok: true
      readonly data: {
        readonly checkedAt: string
        readonly runtimes: readonly PublicRuntimeStatus[]
        /**
         * Whether npm can be run at all.
         *
         * Four of the five runtimes install through it, so its absence is a
         * PRECONDITION, not a failure: a button that cannot work is worse
         * than the truth. This was designed, built into the screen, and then
         * never computed -- so the state could not occur, and a machine
         * without Node got "npm stopped with an error after 1s" from the
         * command the app had just offered to run for it (drive, 2026-09-06,
         * pressing the button for the first time).
         */
        readonly npmPresent?: boolean
        /**
         * True when the npm that will run is the one this app carries, not
         * one on the machine. The buttons work either way; what differs is
         * that a CLI installed this way is reachable from Locust and not
         * from the person's own terminal, which is worth saying on screen
         * rather than leaving to be discovered.
         */
        readonly npmIsBundled?: boolean
        /**
         * npm IS on this machine and did not answer its version check in five
         * seconds -- nvm-windows with no version picked, a corporate wrapper
         * waiting on a proxy.
         *
         * Carried apart from `npmIsBundled` because they are the same VALUE
         * and different SENTENCES: both select the app's own npm, correctly,
         * but only one of them means "Node.js is not on this machine". Fable
         * measured the wrong sentence on a box with Node on PATH and only npm
         * hanging (pass 2, finding 3). The host knew; it stopped saying.
         */
        readonly npmDidNotAnswer?: boolean
      }
    }
  | {
      readonly ok: false
      readonly error: {
        readonly code: 'DISCOVERY_FAILED'
        readonly message: string
      }
    }

export type CodexMissionErrorCode =
  | 'INVALID_PROMPT'
  /**
   * The teammate has reached the monthly limit the person set. Nothing was
   * started or recorded; the message says what was spent and how to go on.
   */
  | 'SPEND_LIMIT_REACHED'
  | 'POLICY_REFUSED'
  | 'RUN_ALREADY_ACTIVE'
  | 'CODEX_UNAVAILABLE'
  | 'RUNTIME_START_FAILED'
  | 'PERSISTENCE_FAILED'
  | 'RUN_NOT_ACTIVE'
  | 'INTERNAL_ERROR'
  /**
   * The run was stopped for a handoff, and the handoff could not proceed. The
   * stop is NOT undone -- nothing here can restart a killed process -- so this
   * error always describes a mission that is now stopped, and the message says
   * so rather than implying the user can simply try again.
   */
  | 'HANDOFF_REFUSED'
  /**
   * No folder is chosen for the teammates to work in. The installed app is
   * launched from its own install folder, which is never a workspace, so a
   * start there is refused until a folder is picked -- it must not quietly
   * edit the app's own files.
   */
  | 'NO_WORKSPACE'
  /**
   * The turn was sent with Cloud, and the repository has no cloud computer yet -- or GitHub has not let Locust make
   * one (0.761). Nothing was started; the window asks the person, with what it costs.
   */
  | 'CLOUD_SETUP_NEEDED'
  /**
   * The turn was sent to the cloud computer for a Claude Code or Codex teammate, and that agent is not signed in
   * there yet (0.768). Nothing was started; the window offers the one-click sign-in.
   */
  | 'CLOUD_SIGN_IN_NEEDED'
  /**
   * A new conversation in a folder like home or `.claude` that the person has not said yes to (0.790). Nothing was
   * started; the words go back in the box, and the window asks once, over it: Choose another folder, or Start here.
   */
  | 'FOLDER_NOT_CONFIRMED'

export interface CodexMissionError {
  readonly code: CodexMissionErrorCode
  readonly message: string
  /**
   * On RUN_ALREADY_ACTIVE only: `pool` when every run slot is taken rather
   * than this teammate being mid-run. Same code, because a person's message
   * waits either way; the relay has to know which, because a reply held for
   * an IDLE teammate never sees that teammate's run end (harness review,
   * 2026-09-24).
   */
  readonly busy?: 'pool'
  /** On CLOUD_SETUP_NEEDED only: which repository, and whether GitHub's permission is what is missing. */
  readonly cloudSetup?: { readonly repo: string; readonly branch?: string; readonly permission?: true }
  /** On CLOUD_SIGN_IN_NEEDED only: which cloud computer, and which agent on it. */
  readonly cloudSignIn?: { readonly computer: string; readonly repo: string; readonly agent: CloudAgent }
  /** On FOLDER_NOT_CONFIRMED only: why the folder is no place for teammates ("your home folder", ".claude, where ..."). */
  readonly folderCaution?: string
  /**
   * On HANDOFF_REFUSED only: the refusal came before anything was stopped,
   * so the mission is still running where it was (M9). Without it the window
   * could not tell "nothing happened" from "stopped, and nothing replaced it".
   */
  readonly untouched?: true
}

/**
 * How much a mission may touch. `ask` is read-only: the agent inspects and
 * explains, and the OS sandbox refuses every write. `accept-edits` lets it edit
 * files inside the workspace folder the host chose, and nowhere else.
 *
 * Consent is given here, at the start, because `codex exec` has no interactive
 * approval channel -- there is no way for the runtime to stop mid-run and ask.
 * Per-action approval needs the experimental app-server protocol.
 */
/**
 * `ask` is read-only. `accept-edits` may write inside the workspace with no
 * per-action prompt. `approve-each` runs on the app-server transport, where the
 * runtime can stop and ask before every consequential action.
 *
 * These are three different bargains, not three intensities, so the composer
 * states the consequence of each rather than only its name.
 */
/**
 * What a mission may do. `plan` is the fourth because planning IS a
 * permission stance, not a switch beside one: it can only ever be chosen
 * where the sandbox already refuses writes, so a separate toggle asked the
 * same question twice in two shapes and could disagree with the answer next
 * to it (design pass, 2026-09-05).
 *
 * `auto` is the fifth, and the only one that has to be switched on before it
 * can be chosen (Colin, 2026-09-06: "there needs to be an auto option ... to
 * allow them to work out of the workspace folder if desired by the user").
 * It runs without asking and is not confined to the workspace folder, which
 * is the one bargain in this list the app cannot take back on the person's
 * behalf -- so the host refuses it unless `WorkspaceSettings.autoMode` is on,
 * whatever the renderer sends.
 */
export type MissionMode = 'ask' | 'accept-edits' | 'approve-each' | 'plan' | 'auto'

/**
 * A model the active runtime actually reports, with the reasoning efforts IT
 * supports. Effort is per model -- offering one a model cannot honour would be
 * a silent no-op, which the design explicitly forbids.
 */
export interface PublicModel {
  readonly id: string
  /**
   * The concrete model id for each effort, when a runtime encodes effort in
   * the id rather than taking a flag. Cursor lists `cursor-grok-4.6-high` and
   * `cursor-grok-4.6-low` as separate models; they are one model here, and
   * this says which id each effort means.
   */
  readonly variants?: Readonly<Record<string, string>>
  /**
   * The level the runtime itself starts this model at, when it says one.
   * Cursor lists that variant under the model's bare name -- "Kimi K3" is
   * `kimi-k3-max` -- and a new route starts there rather than on Locust's
   * own guess (`defaultEffort` in status.ts).
   */
  readonly defaultEffort?: string
  /** The runtime this model belongs to; a model is never offered under another. */
  readonly runtime: MissionRuntimeId
  readonly displayName: string
  readonly description: string
  readonly supportedEfforts: readonly string[]
  /**
   * An older, fixed version -- offered folded under the runtime's current
   * models rather than beside them. Colin, 2026-09-22: "folded claude models
   * is great, accessible but not crowding".
   */
  readonly older?: boolean
  /**
   * A model the person added themselves (0.357): their company's endpoint,
   * or one on this machine, run through OpenCode. The picker lists these in
   * a group of their own, first.
   */
  readonly own?: true
  /** One of the person's own models that only chats (0.358): its runs get no tools, so it cannot read a file (0.810). */
  readonly chatOnly?: true
  /** An agent the person added (0.777, shared/acp-agents.ts): its name, which is its group in the picker. */
  readonly agentName?: string
}

/**
 * A model of the person's own, as the window sees it: never its key, only
 * whether one is kept (main/own-models.ts).
 */
export interface PublicOwnModel {
  readonly ownId: string
  /** What the person called it. */
  readonly name: string
  /** The OpenAI-compatible address, like https://llm.example.com/v1. */
  readonly baseUrl: string
  /** The id the endpoint knows the model by. */
  readonly model: string
  readonly hasKey: boolean
  /** A model that only chats: its teammates talk with it, and it reads and changes no file (0.358). */
  readonly chatOnly: boolean
  readonly createdAt: string
}

export const OWN_MODEL_LIST_CHANNEL = 'ownModels:list'
export const OWN_MODEL_CHAT_ONLY_CHANNEL = 'ownModels:chatOnly'
export const OWN_MODEL_ADD_CHANNEL = 'ownModels:add'
export const OWN_MODEL_REMOVE_CHANNEL = 'ownModels:remove'
export const OWN_MODEL_TEST_CHANNEL = 'ownModels:test'

export interface OwnModelAddRequest {
  readonly name: string
  readonly baseUrl: string
  readonly model: string
  /** Absent for an endpoint that asks for none. Kept only encrypted, and never sent back. */
  readonly key?: string
  /** A model that only chats (0.358). */
  readonly chatOnly?: boolean
}

/** Test a model before adding it (the form's own values), or one already kept (by id, with its kept key). */
export type OwnModelTestRequest =
  | { readonly baseUrl: string; readonly model: string; readonly key?: string }
  | { readonly ownId: string }

export type OwnModelListResponse =
  | { readonly ok: true; readonly data: { readonly models: readonly PublicOwnModel[] } }
  | { readonly ok: false; readonly error: { readonly code: 'OWN_MODELS_UNAVAILABLE'; readonly message: string } }

export type OwnModelMutationResponse =
  | { readonly ok: true; readonly data: { readonly model?: PublicOwnModel } }
  | { readonly ok: false; readonly error: { readonly code: 'OWN_MODEL_REFUSED'; readonly message: string } }

export type OwnModelTestResponse =
  | { readonly ok: true; readonly data: { readonly reached: boolean; readonly said: string; readonly tools?: boolean } }
  | { readonly ok: false; readonly error: { readonly code: 'OWN_MODEL_REFUSED'; readonly message: string } }

export type ModelCatalogResponse =
  | {
      readonly ok: true
      readonly data: {
        readonly models: readonly PublicModel[]
        /**
         * What an account has used, read with the models (0.390): Codex's
         * `account/rateLimits/read`, asked in the same server session as
         * `model/list` -- no turn spent -- in the reading's usual words.
         */
        readonly usageWindows?: Readonly<Record<string, string>>
      }
    }
  | { readonly ok: false; readonly error: { readonly code: 'MODELS_UNAVAILABLE'; readonly message: string } }

/**
 * Workspace-wide settings. `swarm` runs every mission at its model's MAXIMUM
 * supported effort -- which is only meaningful because the catalog reports
 * effort per model, so "maximum" is a real value rather than a guess.
 */
/** The shell layout a person has asked for. `auto` follows the window width. */
export type LayoutPreference = 'auto' | 'compact' | 'wide'
/** How much theatre the boot screen is allowed. */
export type TubePreference = 'full' | 'subtle' | 'off'

/**
 * How big a teammate's reply is set.
 *
 * A real preference, and it exists because I got this wrong on Colin's own
 * screen. 0.198.0 moved the reply from 15px to 18px off a character count --
 * the count was right that 15px rendered 111-character lines, and it could not
 * answer the question it was used to settle, which is how big the type should
 * be. Colin, seeing it on his monitor: "go back to the old text size, this
 * shit looks insane, or you can have it be changeable in settings".
 *
 * So the default is what he had, and the size is his rather than mine.
 * Absent or malformed reads as `standard`, which is 15px.
 */
export type ReplyTextSize = 'standard' | 'large' | 'largest'

/**
 * The metal on the send button.
 *
 * Every option the design agent put forward, on screen rather than in a
 * constant, because Colin asked to see them all before a default is chosen
 * (2026-09-20: *"we can have use all the options the design agent threw at
 * us, we can pick a default soon once we see them all"*). The library ships
 * exactly `chromatic`, `silver` and `gold`; `off` is this app's own.
 *
 * `off` is not only taste. The effect is WebGL, and a person on a tired
 * machine — or one who simply does not want a shader in their composer —
 * should be able to say so without editing anything.
 */
export type MetalPreset = 'off' | 'chromatic' | 'silver' | 'gold'

/**
 * How strong, as words rather than a float.
 *
 * The three the brief and Colin actually named: 0.35 was his starting point,
 * 0.55 the design agent's recommendation, and 1.0 the "aggressive glow to
 * avoid" — so the top of this scale stops short of it.
 */
export type MetalStrength = 'subtle' | 'standard' | 'strong'

/**
 * When it moves.
 *
 * `hover` is the design agent's proposal and the reason is the app's own
 * rule: motion means work is happening, so a permanently shimmering button
 * says "running" on a screen where nothing is. `always` is the version it
 * asked to reject — kept here because a rejection is easier to agree with
 * after seeing it.
 */
export type MetalMotion = 'hover' | 'always'

export interface WorkspaceSettings {
  readonly swarm: boolean
  /** The metal on the send button; `off` removes it entirely. */
  readonly metal?: MetalPreset
  readonly metalStrength?: MetalStrength
  readonly metalMotion?: MetalMotion
  /** The cursor bend — the "gooey" dent that rides the ring. */
  readonly metalBend?: boolean
  /**
   * Every bot wears a screen for a face, its eyes lit code (0.561); off, each
   * keeps its own eyes. On unless it was switched off.
   */
  readonly terminalFaces?: boolean
  /** Every one of our bots is plush faux fur instead of plastic (0.577). Off unless chosen. */
  readonly plush?: boolean
  /**
   * A Claude Code teammate may use the person's own skills, from
   * ~/.claude/skills, as well as the folder's (0.679). Off unless chosen:
   * those were written for their own sessions. Auto finds them regardless,
   * since Auto runs as the person.
   */
  readonly claudeOwnSkills?: boolean
  /**
   * Teammates reply to each other on their own: a share to a teammate starts
   * a run for them, and their answer starts the sender's next turn. On by
   * default -- talking to each other is the point of having more than one --
   * and it ends when a reply has nothing more to say; a hop cap is the
   * backstop that bounds the spend.
   */
  readonly relay: boolean
  /**
   * How many automatic replies one exchange may use before it stops and
   * waits for a person -- the autonomy budget. Six was a constant; the
   * 0.21.2 QA pass (rec. 6) asked for it to be the person's own number.
   * Bounded 1..12 by the host; anything else reads as the default.
   */
  readonly relayHopCap: number
  /**
   * Whether a teammate may INTERRUPT another rather than wait their turn.
   *
   * A teammate runs one mission at a time, so a message that arrives mid-run
   * waits for that run to end. Usually right. Sometimes far too late: the
   * common urgent message is "stop, I am editing that file", and delivering
   * it after the conflicting work is done delivers it after the damage.
   *
   * Off until a person turns it on, because it SPENDS and DISCARDS: the
   * recipient's turn is stopped where it stands. On, a share that carries
   * `when="now"` stops the recipient's run so the message is taken next;
   * anything else still waits. Read when a message is relayed, never cached.
   */
  readonly interrupt: boolean
  /** What happens to a memory a teammate writes. Absent or malformed reads as the default. */
  readonly memoryMode: MemoryMode
  /**
   * Whether the Auto mode may be chosen at all. Off until a person turns it
   * on, and the host checks it again when a run starts -- a mode that lets a
   * runtime change files anywhere on the machine is not something a stale
   * window or a saved routine gets to decide.
   */
  readonly autoMode: boolean
  /**
   * Ask before every connector call, instead of allowing the ones the person
   * already has.
   *
   * Off is Colin's ruling and the ordinary state: a connector the person's
   * Claude Code can reach is allowed without asking. On sends no allow rules,
   * so every connector call stops the run and raises the approval card --
   * the way every other client behaves -- with "always" remembered per
   * connector until the mission ends. Checked when a run starts, never
   * cached; absent or malformed reads as off.
   */
  readonly askConnectors: boolean
  /**
   * Ask the teammate to keep a todo list as it works, so the board fills in
   * while the mission runs.
   *
   * Off by default and opt in, because it spends tokens on bookkeeping and
   * changes how a runtime narrates itself -- worth it when you are watching,
   * noise when you are not. Only ever sent to a runtime that HAS such a tool
   * (`RUNTIMES_THAT_KEEP_A_TODO_LIST`); Claude Code has none, so the sentence
   * is never added to its briefing whatever this says. Absent or malformed
   * reads as off.
   */
  readonly keepATodoList: boolean
  /**
   * Which shell layout to draw: the full sidebar, the compact avatar rail, or
   * whichever the window width calls for. Colin asked for the layout to be a
   * choice as well as a consequence of window size (2026-09-07). Absent or
   * malformed reads as `auto`, which is what it did before the choice existed.
   */
  readonly layout: LayoutPreference
  /**
   * How much of the boot screen to draw while runtimes are found.
   *
   * A real preference because a person will see this on every launch --
   * hundreds of times. `full` is the phosphor tube, `subtle` keeps the wash
   * and drops the flicker and glare, `off` skips the ceremony entirely and
   * goes straight to the pane. Absent or malformed reads as `full`.
   */
  readonly tube: TubePreference
  /**
   * How big to set a teammate's reply. Absent or malformed reads as
   * `standard` -- see `ReplyTextSize`, which carries the reason this is a
   * setting at all.
   */
  readonly replySize: ReplyTextSize
  /**
   * A3.3: the command Locust runs in a teammate's folder after a turn that
   * changed files -- THIS folder's, as the window sees it. The person's
   * own words in Settings, never read from the project: running a command
   * the repository names after every turn is the hole 0.341 closed.
   */
  readonly checkCommand?: string
  /** Every folder's check command, by workspace id. Kept by the host. */
  readonly checkCommands?: Readonly<Record<string, string>>
  /**
   * When THIS folder's check fails after a turn the person sent (0.812, shared/check-fix.ts): send it back to the same
   * teammate to fix, at most this many times. Absent or 0: show it, and wait for the person, as before.
   */
  readonly checkFixTries?: number
  /** Every folder's most fixes, by workspace id. Kept by the host. */
  readonly checkFixes?: Readonly<Record<string, number>>
  /**
   * About you: the person's standing note, read by every teammate before
   * every run (shared/about-you.ts). Absent means none. Only the person
   * writes it, on the Memory screen.
   */
  readonly aboutYou?: string
  /** Lines teammates suggested for it, waiting for the person (0.424). */
  readonly aboutYouSuggestions?: readonly AboutYouSuggestion[]
  /**
   * The model a new chat with nobody starts on: the last one picked there
   * (0.552). Claude Code's `/model` sets the default for new sessions; a
   * conversation already going keeps its own.
   */
  readonly newChatRoute?: TeammateRoute
  /**
   * When an agent's plan runs out mid-task, the agent that carries the work on, and whether it does so without a
   * press (0.774, shared/carry-on.ts). Absent: the stopped run's card offers it. Written as null to clear it.
   */
  readonly planRunsOut?: PlanRunsOut | null
}

/*
 * The budget is a BACKSTOP, not a conversation length.
 *
 * It was six, and six was firing as the ordinary way an exchange ended: five
 * runs of a one-word question went 6, 6, 3, 6, 7 hops (MEASURED 2026-09-11).
 * A limit that fires in the normal case is not a backstop, it is a timer, and
 * a teammate cut off mid-thought is the worse failure of the two.
 *
 * Colin, the same day: "I honestly think we should just let the teammates talk
 * until it comes to a natural end. That just seems like the smoothest
 * integration." Right about the goal. The thing that stopped it being true was
 * that exchanges were not ENDING -- the brief now names what a reply costs,
 * and three runs after it went 2, 5, 3.
 *
 * So: generous enough that a person never meets it while the work is real,
 * small enough that two models stuck in a loop at three in the morning do not
 * empty an account. Twelve is roughly four times the observed median, and the
 * spend it bounds is now the WHOLE exchange rather than one chain of it.
 */
export const DEFAULT_RELAY_HOP_CAP = 12
export const MIN_RELAY_HOP_CAP = 1
export const MAX_RELAY_HOP_CAP = 24

/** What the runtime is asking permission to do. */
/**
 * `connector` is a Claude Code run asking to use one of the person's MCP
 * tools. It arrives through Locust's own permission host rather than the
 * app-server protocol, but it is answered by the same card, with the same
 * three decisions -- and "always" is remembered for that connector on that
 * run, the way every other client does it.
 */
export type MissionApprovalKind = 'command' | 'file-change' | 'question' | 'connector'

export interface MissionApprovalRequest {
  readonly approvalId: string
  readonly runId: string
  readonly missionId: string
  readonly kind: MissionApprovalKind
  /** One line naming the action, safe to show. */
  readonly summary: string
  /** The exact command or change, already bounded. Empty when there is none. */
  readonly detail: string
  /** Where it would happen. */
  readonly cwd: string | null
  readonly requestedAt: string
  /**
   * Who is asking. The card used to print "Codex CLI" as a constant, which
   * was true for as long as Codex was the only runtime that could ask; a
   * Claude Code connector permission wore the wrong name in its first drive
   * (2026-09-10). Absent on a record from before this field: read as Codex.
   */
  readonly runtime?: MissionRuntimeId
  /**
   * For a file change: the unified diff Codex attached to the item the
   * approval is about, bounded like a ledger patch. Absent when the runtime
   * sent none -- the card then says what it was told and no more.
   */
  readonly patch?: ApprovalPatch
  /**
   * For a question: what was actually asked, structured.
   *
   * Present only on `kind: 'question'`. A command or a file change is
   * authorized, not answered, so it carries none.
   */
  readonly questions?: readonly MissionQuestion[]
  /**
   * Whether the runtime is waiting on this before it can go on.
   *
   * The protocol says so per request and Locust assumed it of every one. A
   * non-blocking question is one the run continues past, and drawing it as a
   * stop-everything card would be a claim about the run that is not true.
   */
  readonly blocking?: boolean
  /**
   * Where the question has to be answered, when it cannot be answered here.
   *
   * Antigravity's questions are answered through its own server; when Locust
   * could not find the waiting step there, the card still shows the question
   * and says to answer it in this app's window, with no buttons that would
   * pretend otherwise.
   */
  readonly answerIn?: string
  /** A question the person may decline, as Antigravity's own card offers. */
  readonly skippable?: boolean
  /**
   * What "Always allow this session" lets through without asking again, in
   * words (QA-2026-09-29 round 2, R35): "list_issues on github, and no other
   * tool", "any command matching echo *". Absent: said generally.
   */
  readonly alwaysCovers?: string
  /**
   * What an Always on this card remembers for the run, made by the host that
   * raised it (0.616, shared/who-decides.ts): `claude:Bash`, an exact command
   * for Copilot, OpenCode's own patterns. The main process keeps the run's
   * keys and decides every later request with them, after the saved rules.
   * Absent where the runtime keeps its own Always (Codex).
   */
  readonly alwaysKey?: string
  /** Why a card is shown although an earlier Always might have covered it (0.616): a command that reaches other programs. */
  readonly askedAgain?: string
  /**
   * The card's Data sent and Reversible lines, when the asking route knows
   * better than the kind does (R15, R37): a fetch is not a command, and a
   * file outside the folder is not under its version control.
   */
  readonly dataSentSays?: string
  readonly reversibleSays?: string
  /**
   * The call this request is about, by the runtime's own id for it (Claude
   * Code's `tool_use_id`, which its permission bridge forwards). The thread
   * matches it to the call's row, and a call made by a helper names that
   * helper on the card (helper visibility, 2026-10-05). Absent where the
   * runtime does not say.
   */
  readonly toolUseId?: string
}

export interface ApprovalPatch {
  readonly text: string
  readonly added: number
  readonly removed: number
  readonly truncated: boolean
}

/**
 * `approve-once` allows this action only. `approve-always` allows matching
 * actions for the rest of the session. `deny` refuses it. There is deliberately
 * no "always, forever" -- a durable grant is a Settings decision, not something
 * to hand over mid-run under time pressure.
 */
export type MissionApprovalDecision = 'approve-once' | 'approve-always' | 'deny'

/** One choice a question offers. The protocol gives a label and a description, and no id. */
export interface MissionQuestionOption {
  readonly label: string
  readonly description: string | null
}

/**
 * One question a runtime asked, as the protocol actually sends it.
 *
 * `item/tool/requestUserInput` carries a `questions` ARRAY, each entry with its
 * own id, header, text, flags and nullable options. Locust read singular
 * `params.question` / `prompt` / `message` and found none of them, so the card
 * showed "Answer a question" with no question -- verified against the schema
 * codex-cli 0.153.0 generates for itself (Astra, 2026-09-09).
 */
export interface MissionQuestion {
  /** The key an answer is filed under. Not an index, not the request id. */
  readonly id: string
  readonly header: string | null
  readonly question: string
  /** Absent or empty means free text only. */
  readonly options: readonly MissionQuestionOption[]
  /** The person may answer with something not offered. */
  readonly isOther: boolean
  /** The answer is sensitive: never logged, never persisted in the clear. */
  readonly isSecret: boolean
}

/**
 * What the person said back.
 *
 * A question is NOT an authorization, and this is where that stops being a
 * comment and becomes a type. A command or a file change is answered with a
 * decision; a question is answered with ANSWERS, keyed by question id, each a
 * list of strings -- a chosen option's literal label, or free text, or both.
 *
 * Locust used to send `{ decision: 'accept' }` for all three buttons on a
 * question card. The server cannot deserialize that as an answer, logs the
 * failure, and submits an EMPTY answer map -- so the person's selection was
 * discarded and the model was told they had said nothing. "Always allow this
 * session" was the worst of the three, because there is no session-grant field
 * for a question at all: it promised something the protocol has no way to mean.
 */
export type MissionApprovalAnswer =
  | {
      readonly approvalId: string
      readonly decision: MissionApprovalDecision
      /**
       * Why the person denied it, in their words (0.374). Only on a denial.
       * A bare "denied" left a teammate to guess, and the guess was usually
       * the same action by another route; Vibe Kanban asks "Let the agent
       * know why" (docs/RESEARCH-2026-09-26-ROOMS-AND-PEERS.md).
       */
      readonly reason?: string
    }
  | {
      readonly approvalId: string
      /** Keyed by `MissionQuestion.id`; each value is that question's answers. */
      readonly answers: Readonly<Record<string, readonly string[]>>
    }

export interface CodexMissionStartRequest {
  readonly scoutCardId?: string
  /** Host picker/clone identities, never paths. Omitted means keep this conversation's references. */
  readonly referenceIds?: readonly string[]
  readonly account?: string
  readonly prompt: string
  readonly mode?: MissionMode
  readonly runtime?: MissionRuntimeId
  /** A model id from the catalog, or omitted for the account default. */
  readonly model?: string
  /** Only meaningful when the chosen model reports supporting it. */
  readonly effort?: string
  /**
   * The teammate this mission is messaged to. Decides who the mission belongs
   * to, whose waiting workroom messages it is shown, and under whose name its
   * findings are shared. Absent for a mission that belongs to nobody.
   */
  readonly teammateId?: string
  /**
   * The mission this one replies to. The host looks up that mission's own
   * session handle and resumes it, so a second message is the next turn of a
   * conversation rather than a stranger arriving mid-thought. A mission that
   * cannot be resumed is refused rather than silently started blank.
   */
  readonly followUpOf?: string
  /**
   * Filled from a GitHub issue. The host checks the branch out in a worktree
   * teammate's copy and remembers the issue. The words in `prompt` stay the
   * issue's own text. Nothing here is sent until the person presses Send.
   */
  readonly issue?: StartedIssue
  /**
   * Started again from an edited earlier message (0.498). `followUpOf` is
   * then the turn BEFORE the edited one (absent when the first message was
   * edited), and `tip` the conversation's latest turn, which the new branch
   * replaces as the teammate's current conversation.
   */
  /** `setAside`: the turns the edit sets aside (0.512); their unread messages are never delivered. */
  readonly rewind?: { readonly tip: string; readonly setAside?: readonly string[] }
  /** This run's mode is for it alone (0.514, a review in Ask): the teammate's saved route is not changed. */
  readonly keepSavedRoute?: true
  /** Raw picker identity before an effort choice expands a model variant. */
  readonly modelChoice?: string
  /** Only a person's picker change this session, addressed to this teammate. */
  readonly routeOverrideFor?: string
  /** A reply on another runtime: brief sections the person chose to leave out (0.527). */
  readonly leaveOut?: readonly string[]
  /**
   * YOUR CLOUD COMPUTER (0.761): this turn runs on the repository's GitHub Codespace, not on this computer.
   * `cloudSetUp`: the person pressed Set it up, so the repository's first one may be made.
   */
  readonly cloud?: true
  readonly cloudSetUp?: true
}

/**
 * What the renderer needs to say which route a mission is on. Shared by a live
 * start response and a mission restored from the ledger, because a restored
 * mission may be on a runtime this build cannot start.
 */
export interface MissionRouteSummary {
  readonly account?: string
  readonly runId: string
  readonly missionId: string
  readonly runtime: MissionRuntimeId
  readonly model: string
  readonly resolvedRouteId: string
  readonly cliVersion: string | null
  /**
   * What the run was actually allowed to do. Carried here so the UI states the
   * real posture of THIS run rather than whatever mode the composer shows now.
   */
  readonly sandbox: 'read-only' | 'workspace-write' | 'full-access'
  /**
   * What was ASKED for, beside what it was allowed: Approve-each shares
   * Edit's sandbox and stops before each act, so the sandbox alone cannot
   * say which of the two a run was (the Activity panel's rows, 0.359).
   */
  readonly mode?: MissionMode
  /**
   * The host looked at the folder before and after this run (git, a folder
   * it had to itself), so a file a COMMAND wrote is listed as changed too.
   * Absent, only what the runtime reported is known -- and the Artifacts tab
   * must not say "changed no files" about a run it could not see (0.364).
   */
  readonly watchesDisk?: true
}

/**
 * A mission start receipt. `runtime` is the union rather than a literal now
 * that Claude can own a run: narrowing it here would make a real Claude start
 * a type error instead of a supported route.
 */
export interface CodexMissionStartData extends MissionRouteSummary {
  /** Set when this run continued an earlier mission's conversation. */
  /**
   * The turn this one continues. `runtimeThreadId` is absent when that turn
   * left no session to resume -- the conversation still continues, the model
   * just does not carry it.
   */
  readonly followsUp?: { readonly missionId: string; readonly runtimeThreadId?: string }
  /**
   * Set when this reply went to ANOTHER runtime than the turn it answers: the
   * host reconciled that turn and briefed this runtime from it (a
   * 'route-switch' continuation). Without it the window could not tell a
   * switch from an ordinary reply, and drew no seam until the conversation
   * was rebuilt from the record (drive-runtime-switch, packaged 0.309).
   */
  readonly switchedFrom?: {
    readonly missionId: string
    readonly runtime: MissionRuntimeId
    /** The model the turn before the seam ran on, for the divider to name (0.779). */
    readonly model?: string
    /** Actions that had started and never reported back when that turn ended. */
    readonly unsettledCount: number
    /** Sections of the briefing dropped to fit the prompt bound, if any. */
    readonly omittedBriefing: readonly string[]
    /** Sections the person chose to leave out (0.527). */
    readonly leftOutByYou?: readonly string[]
  }
  /**
   * The project this new conversation moved the window to (0.814, main/named-folder.ts): its message named a file
   * inside it, from a folder like .claude or another checkout of the same repository.
   */
  readonly movedToFolder?: { readonly path: string; readonly name: string }
  /** Workroom messages quoted into this mission's prompt, oldest first. */
  readonly peerMessages: readonly PublicPeerMessage[]
  /**
   * True when the workroom could not be read at start. The mission still ran,
   * with no teammate messages; whatever was waiting is still waiting.
   */
  readonly peerDeliveryFailed: boolean
}

export type CodexMissionStartResponse =
  | { readonly ok: true; readonly data: CodexMissionStartData }
  | { readonly ok: false; readonly error: CodexMissionError }

export interface CodexMissionCancelRequest {
  readonly runId: string
}

/**
 * Move work from one runtime to another mid-mission.
 *
 * This is deliberately NOT modelled as "the same mission changes runtime". A
 * mission records ONE runtime and every event must agree with it, so a switch
 * produces a NEW mission that continues from a checkpoint of the old one --
 * which is also what actually happened: two runs, with a reconciliation
 * between them.
 */
export interface MissionHandoffRequest {
  readonly account?: string
  readonly runId: string
  readonly runtime: MissionRuntimeId
  readonly mode: MissionMode
  readonly model?: string
  readonly effort?: string
}

/**
 * Picking a stopped mission back up. Named by MISSION rather than by run,
 * because the run it continues is over -- that is the whole case.
 */
export interface MissionResumeRequest {
  readonly account?: string
  readonly missionId: string
  readonly runtime: MissionRuntimeId
  readonly mode: MissionMode
  readonly model?: string
  readonly effort?: string
}

export interface MissionHandoffData extends CodexMissionStartData {
  /** The mission this one continues, and the checkpoint it resumed from. */
  readonly continuesFrom: {
    readonly missionId: string
    readonly checkpointEpoch: number
  }
  /**
   * How much the briefing could promise the new runtime. `safe` means nothing
   * was in flight when the old run stopped; `approval-required` means some
   * actions started and never reported back, and the new run has been told to
   * verify them before building on them.
   */
  readonly resumeSafety: 'safe' | 'approval-required'
  /** Sections of the briefing dropped to fit the prompt bound, if any. */
  readonly omittedBriefing: readonly string[]
  /**
   * How many actions had started and not reported back when the old run
   * stopped. The divider states this rather than a reassuring summary: it is
   * the number the person needs to decide whether to trust what follows.
   */
  readonly unsettledCount: number
}

export type MissionHandoffResponse =
  | { readonly ok: true; readonly data: MissionHandoffData }
  | { readonly ok: false; readonly error: CodexMissionError }

export type CodexMissionCancelResponse =
  | {
      readonly ok: true
      readonly data: {
        readonly runId: string
        readonly state: 'cancellation-requested'
      }
    }
  | { readonly ok: false; readonly error: CodexMissionError }

/** The phases of a start, in order (0.602): what the live line says while nothing has arrived. */
/** The cloud computer's four (0.761): made, woken, the agent installed there, this folder brought over. */
export type StartPhase = 'looking' | 'briefing' | 'reading-folder' | 'starting-runtime' | 'runtime-first-setup' | 'cloud-making' | 'cloud-waking' | 'cloud-installing' | 'cloud-bringing'

export type CodexMissionUpdate =
  | {
      /** Durable text fragments, kept in order and applied in one update. */
      readonly kind: 'message-deltas'
      readonly runId: string
      readonly missionId: string
      readonly events: readonly Extract<NormalizedRuntimeEvent, { readonly type: 'message.delta' }>[]
    }
  | {
      /** A turn on the cloud computer: where it ran, and what became of its change (0.761). */
      readonly kind: 'cloud-turn'
      readonly turn: PublicCloudTurn
    }
  | {
      /** Setting up the cloud computer, step by step, for the dialog (0.761); `waiting`: a sign-in's page is open (0.768). */
      readonly kind: 'cloud-setup'
      readonly step: CloudSetupStep
      readonly waiting?: true
    }
  | {
      /** An agent being signed in on a cloud computer, step by step (0.768). */
      readonly kind: 'cloud-sign-in'
      readonly name: string
      readonly agent: CloudAgent
      readonly step: CloudSignInStep
    }
  | {
      /** A comparison run again (0.763, your own benchmark): the comparisons that changed, and where each batch is. */
      readonly kind: 'compare-reruns'
      readonly compares: readonly PublicCompare[]
      readonly progress: readonly RerunProgress[]
    }
  | {
      /** The start's phase (0.602), sent while the window's run is still "starting" and before it knows its ids. */
      readonly kind: 'start-phase'
      readonly runtime: MissionRuntimeId
      readonly teammateId?: string
      readonly phase: StartPhase
      readonly at: string
    }
  | {
      readonly kind: 'event'
      readonly runId: string
      readonly missionId: string
      readonly event: NormalizedRuntimeEvent
    }
  | {
      readonly kind: 'transport-error'
      readonly runId: string
      readonly missionId: string
      readonly error: {
        readonly code: 'RUNTIME_TRANSPORT_FAILED'
        readonly message: string
      }
    }
  | {
      readonly kind: 'persistence-error'
      readonly runId: string
      readonly missionId: string
      readonly error: {
        readonly code: 'MISSION_PERSISTENCE_FAILED'
        readonly message: string
      }
    }
  /** The mission's work posted a message to another teammate. */
  | {
      readonly kind: 'peer-message'
      readonly runId: string
      readonly missionId: string
      readonly message: PublicPeerMessage
    }
  /** The mission asked to share something and the host could not honour it. */
  | {
      readonly kind: 'peer-share-failed'
      readonly runId: string
      readonly missionId: string
      readonly message: string
    }
  /**
   * A3.3: the person's check, run after this turn changed files. Only what
   * is new since the last check in that folder is carried.
   */
  | {
      readonly kind: 'edit-check'
      readonly runId: string
      readonly missionId: string
      readonly command: string
      readonly outcome: 'passed' | 'failed' | 'timed-out' | 'could-not-run'
      readonly newLines: readonly string[]
      readonly unchanged: boolean
      readonly first: boolean
    }
  /** That check has started (0.811): its command, until `edit-check` says how it ended. */
  | {
      readonly kind: 'edit-check-started'
      readonly runId: string
      readonly missionId: string
      readonly command: string
    }
  /**
   * The host has decided on a reply and is starting it.
   *
   * Sent BEFORE the runtime exists, and paired with `relay-start-settled`.
   * `mission-started` cannot carry this: it needs a run id, which means
   * waiting for a cold runtime to boot -- and for those seconds the window
   * was told nothing at all, so every row read idle and a room in the middle
   * of an argument looked finished (MEASURED 2026-09-11).
   *
   * One per teammate at a time, because a teammate runs one mission at a
   * time; `teammateId` is therefore the whole key.
   */
  | {
      readonly kind: 'relay-starting'
      readonly teammateId: string
      readonly name: string
      /** The mission whose message this will answer, so a room can place the turn. */
      readonly answering: string
      readonly hop: number
    }
  /** That start finished, however it finished. The placeholder goes. */
  | {
      readonly kind: 'relay-start-settled'
      readonly teammateId: string
    }
  /**
   * The host started a run this renderer did not ask for: a teammate
   * replying on their own. Carries everything a start response would, so
   * the renderer adopts it exactly as it adopts its own.
   */
  | {
      readonly kind: 'mission-started'
      readonly runId: string
      readonly missionId: string
      /** Absent for a run of nobody's: a comparison started with no teammate (0.441). */
      readonly teammateId?: string
      readonly prompt: string
      readonly data: CodexMissionStartData
      /**
       * Who started it: a relay hop of an exchange, one step of a routine
       * being replayed, or a person's post to a room. The room kind lives
       * only in this live update and the window -- the ledger records a
       * room post's missions as ordinary missions of their teammates.
       * Absent: the person's own next message, sent from their phone (0.775) -- as their record says it.
       */
      readonly startedBy?:
        | { readonly kind: 'mcp'; readonly via?: 'voice' }
        | { readonly kind: 'relay'; readonly hop: number }
        | { readonly kind: 'routine'; readonly routineId: string; readonly step: number }
        | { readonly kind: 'room'; readonly roomId: string; readonly postId: string }
        // A teammate tagged from a conversation (0.438): the person's words, live only, like a room's.
        | { readonly kind: 'tag' }
        /** A comparison's column (0.441): drawn in the comparison, never focused on its own. */
        | { readonly kind: 'compare'; readonly compareId: string; readonly slot: CompareSlotId }
        /** A question on the side of a conversation (0.461): drawn in the side panel, never in the sidebar. */
        | { readonly kind: 'side'; readonly of: string; readonly question: number }
        /** A comparison's judge (0.520): drawn in the comparison, never as a conversation. */
        | { readonly kind: 'judge'; readonly compareId: string }
        /** One of Scout's looks (0.756): its answer fills in the cards on Home, never drawn as a conversation. */
        | { readonly kind: 'scout' }
        /** A commit message written from the changes (0.771): its answer fills the Commit box, never a conversation. */
        | { readonly kind: 'commit-draft' }
        /** A walk through a change (0.771): its answer is the Changes panel's, never a conversation. */
        | { readonly kind: 'walkthrough' }
        /** The turn before ran out of `from`'s plan, and Locust carried the work on (0.774). */
        | { readonly kind: 'carry-on'; readonly from: MissionRuntimeId }
        /** The person's check failed after the turn before, and Locust sent it back to fix (0.812, shared/check-fix.ts). */
        | { readonly kind: 'check-fix'; readonly fix: number; readonly of: number; readonly command: string }
      /**
       * Present when this run is the newest turn of the teammate's hub (see
       * `PublicTeammate.hubMissionId`), so the roster on screen learns it
       * without a refresh -- the face has to open the hub from this moment.
       */
      readonly hubMissionId?: string
    }
  /**
   * A post was written to a room, before anybody has been asked.
   *
   * The post has existed before the first run since the queue landed, but
   * nothing SAID so: the window learned about it either from the first
   * `mission-started` -- which waits for a cold runtime to boot -- or from
   * the post's own response, which lands after every member has been tried.
   * So a person pressed Post and watched "Nothing posted yet" with their own
   * words still in the box (Colin, 2026-09-11).
   */
  | {
      readonly kind: 'room-posted'
      readonly roomId: string
      readonly postId: string
    }
  /**
   * A room's board moved because a teammate's reply moved it. The window
   * re-reads the room; `message` is the host's one-line account, for the
   * room screen and, later, a toast.
   */
  | {
      readonly kind: 'room-changed'
      readonly roomId: string
      readonly roomName: string
      readonly message: string
    }
  /** A runtime listed a different set of its own commands; the `/` menu re-reads them (0.426). */
  | { readonly kind: 'runtime-commands-changed' }
  /** A runtime listed different models of its own (Claude Code's handshake, 0.697); the picker re-reads the catalog. */
  | { readonly kind: 'models-changed' }
  /** A background turn started, moved or ended (W10): the list is read again. */
  | { readonly kind: 'background-changed' }
  /** A teammate's reply changed the team's memory; the Memory screen and sidebar re-read it. */
  | {
      readonly kind: 'memory-changed'
      readonly by: string
      readonly kept: readonly string[]
      readonly proposed: readonly string[]
      readonly forgotten: readonly string[]
      /**
       * Memories that REPLACED an earlier one filed under the same name.
       * Separate from `kept` on purpose: adding a fact and changing one a
       * person may already have acted on are different events, and folding
       * them together lets a memory move under them without a word.
       */
      readonly rewritten?: readonly string[]
      /**
       * In ask mode, a CHANGE to a kept memory waiting for the person: the
       * new wording of a rewrite, the words of a memory it wants forgotten
       * (0.315). Not `proposed`: "wants to remember" would misname both.
       */
      readonly proposedChanges?: readonly string[]
      readonly proposedForgets?: readonly string[]
      /** A tidy pass's suggestions made proposals, and the ones refused with why (A1.2). */
      readonly proposedTidy?: number
      readonly tidyRefused?: readonly string[]
      /** Lines suggested for the person's About-you note, now waiting for them (0.424). */
      readonly aboutYouSuggested?: readonly string[]
    }
  /** Why a teammate did NOT reply on their own, said in the thread that shared. */
  | {
      readonly kind: 'relay-notice'
      readonly runId: string
      readonly missionId: string
      readonly message: string
      /**
       * `info` for how an exchange is going -- waiting on two teammates, one
       * replied -- and `warning` (the default) when something did not happen
       * or a person may need to act. Every notice was amber, so the ordinary
       * run of a Chief of Staff's first delegation read as five warnings
       * (Rook's first starter, packaged 0.365).
       */
      readonly level?: 'info' | 'warning'
    }
  | {
      /**
       * A scheduled routine that would not start. It has no run and no
       * mission -- nothing was created -- so it cannot travel as a run's
       * notice, which is why it went unsaid: the runner held it off for an
       * hour and tried again, silently, for as long as the refusal lasted
       * (QA, 2026-09-06; `index.ts` already had a comment admitting it).
       */
      readonly kind: 'routine-blocked'
      readonly routineId: string
      readonly name: string
      readonly message: string
      /** When it will be tried again, so the notice can say so. */
      readonly retryAt: string
    }
  | { readonly kind: 'routine-recovery-changed' }

/**
 * A checkpoint as the renderer may see it. Deliberately narrower than the
 * ledger's record: the receipt card needs to say how many checkpoints exist,
 * which was last, and what was left unsettled -- it does not need the
 * transcript digest or the assistant summary, and neither belongs on an IPC
 * surface that exists to render a status card.
 */
export interface PublicMissionCheckpoint {
  readonly epoch: number
  readonly reason: string
  readonly resumeSafety: 'safe' | 'approval-required' | 'unsafe'
  readonly safetyReason: string
  readonly createdAt: string
  readonly unsettledActions: readonly { readonly itemId: string; readonly name: string }[]
}

export interface PublicRecoveredMission {
  readonly missionId: string
  readonly runId: string
  /** The folder the mission ran in, as a stable id derived from its path. */
  readonly workspaceId: string
  readonly prompt: string
  // History spans every runtime a mission could have run under, so this is the
  // union even while Codex is the only one that can be started today. Pinning
  // it to a literal would make recovered Claude missions a type error rather
  // than a missing feature.
  readonly runtime: MissionRuntimeId
  readonly model: string
  readonly requestedRouteId: string
  readonly resolvedRouteId: string
  readonly cliVersion: string | null
  readonly createdAt: string
  readonly lastUpdatedAt: string
  readonly phase: 'completed' | 'failed' | 'cancelled' | 'interrupted'
  readonly events: readonly NormalizedRuntimeEvent[]
  readonly eventCount: number
  readonly eventsTruncated: boolean
  /**
   * What the run cost in MONEY -- dollars a runtime priced, or Copilot's
   * premium requests -- read by the host from the whole record. Present on a
   * row sent without its events too, so a total over rows is a total over
   * every row (shared/spend.ts). Absent when the run reported no money.
   */
  readonly money?: Spend
  /**
   * A fingerprint of this record exactly as it was sent, on a record that came
   * with its events. The window hands it back on its next history read, and a
   * record that has not changed since comes back without them (`eventsKept`).
   */
  readonly digest?: string
  /**
   * The events are the ones the window already holds under `digest`: keep
   * those. Set only on a record the window said it had.
   */
  readonly eventsKept?: boolean
  readonly hostFailureMessage?: string
  readonly integrityIssueCount: number
  readonly sandbox: 'read-only' | 'workspace-write' | 'full-access'
  /** What was asked for. Absent on missions recorded before schema 15. */
  readonly mode?: MissionMode
  readonly checkpoints: readonly PublicMissionCheckpoint[]
  /** Workroom messages this mission received or posted, in ledger order. */
  readonly peerMessages: readonly PublicPeerMessage[]
  /**
   * A3.3: what the person's check said after this turn, the newest one. Absent
   * when no check ran, or on a mission recorded before schema 17.
   */
  readonly editCheck?: {
    readonly command: string
    readonly outcome: 'passed' | 'failed' | 'timed-out' | 'could-not-run'
    readonly newLines: readonly string[]
    readonly unchanged: boolean
    readonly first: boolean
  }
  /**
   * Set when this mission continued another after a route switch. The
   * renderer uses it to draw the pair as one thread under a divider -- and to
   * show the ROOT mission's prompt, because this mission's own recorded prompt
   * is the machine-written briefing.
   */
  readonly continuesFrom?: {
    readonly missionId: string
    readonly checkpointEpoch: number
    /** `route-switch` is a handoff; `follow-up` is the next turn of one conversation. */
    readonly reason: 'route-switch' | 'follow-up'
    /** An edit of an earlier message (0.498): an earlier version followed the same turn. */
    readonly edited?: true
    /** What a handoff's brief left out to fit (0.519), by section name. */
    readonly leftOut?: readonly string[]
    /** What the person chose to leave out of it (0.527), by section name. */
    readonly leftOutByYou?: readonly string[]
  }
  /**
   * Set when the HOST started this run rather than a person -- today, one
   * teammate answering another. Its `prompt` is then instructions the host
   * wrote to a runtime, not a sentence anybody would recognise as their own,
   * which is why the renderer must never title a mission with it.
   */
  readonly startedBy?:
    | { readonly kind: 'mcp'; readonly via?: 'voice' }
    | {
        readonly kind: 'relay'
        /** Which automatic turn of the exchange this is, counting from 1. */
        readonly hop: number
      }
    | {
        /** Picking a mission back up from a checkpoint after an interruption. */
        readonly kind: 'resume'
        readonly epoch: number
      }
    | {
        /** One step of a routine a person saved, replayed by the host. */
        readonly kind: 'routine'
        readonly routineId: string
        readonly step: number
      }
    | {
        /**
         * A turn the person had in the runtime's own terminal, on this
         * conversation's session, brought back into the record (0.391).
         * Its prompt is the person's own words; Locust did not run it.
         */
        readonly kind: 'terminal'
        readonly exchange: number
      }
    | {
        /**
         * A question asked on the side of a conversation (0.461): read-only,
         * on a fork of the session of `of`. It is not a turn of that
         * conversation, and the sidebar never lists it as one.
         */
        readonly kind: 'side'
        readonly of: string
        readonly question: number
      }
    | {
        /** The turn before ran out of `from`'s plan, and Locust carried the work on (0.774). Nobody typed it. */
        readonly kind: 'carry-on'
        readonly from: MissionRuntimeId
      }
    | {
        /**
         * The person's check failed after the turn before, and Locust sent the failure back to the same teammate
         * (0.812, shared/check-fix.ts): fix `fix` of at most `of`. Nobody typed it.
         */
        readonly kind: 'check-fix'
        readonly fix: number
        readonly of: number
        readonly command: string
      }
}

/** H3: one mission, projected as history projects the newest ones. */
export type MissionReadResponse =
  | { readonly ok: true; readonly data: { readonly mission: PublicRecoveredMission } }
  | { readonly ok: false; readonly error: { readonly code: 'MISSION_UNAVAILABLE'; readonly message: string } }

export type MissionHistoryResponse =
  | {
      readonly ok: true
      readonly data: {
        readonly missions: readonly PublicRecoveredMission[]
        /** Host-owned processes, separate from a ledger with no terminal receipt yet. */
        readonly liveMissionIds?: readonly string[]
        /** The folder this window is working in; missions elsewhere are not its own. */
        readonly currentWorkspaceId: string
        readonly issueCount: number
        /**
         * Ledger files that raised an issue and produced no mission at all.
         *
         * Separate from `issueCount` because the screen needs a count of
         * FILES, not of issues: one badly damaged file raises several. And
         * separate from the per-mission counts because these files have no
         * mission to hang a count on -- which is exactly why the Missions
         * header used to read "ledger verified" for a ledger it could not
         * read (Astra, 2026-09-08).
         */
        readonly unreadableCount: number
        /**
         * How many missions the ledger holds, when that is more than were
         * listed (QA-2026-09-29 round 2, R28). Absent when all were listed.
         */
        readonly totalMissions?: number
        /** How many of them were listed, when `totalMissions` is said. */
        readonly listedMissions?: number
        /**
         * Runtimes whose most recent word, across every mission in the
         * ledger, was that the account is out of quota -- with that word.
         * Derived here rather than remembered by the window, because the
         * window forgets on reload and the ledger does not: the 0.21.2 QA
         * pass reloaded past a Codex limit and watched AT LIMIT turn back
         * into READY with no successful run in between.
         */
        readonly limitedRuntimes: Readonly<Record<string, string>>
        /** The latest still-allowed rate-limit reading per runtime ("5-hour window 35% used · resets …"). */
        readonly usageWindows: Readonly<Record<string, string>>
      }
    }
  | {
      readonly ok: false
      readonly error: {
        readonly code: 'HISTORY_UNAVAILABLE'
        readonly message: string
      }
    }

export type MissionDeleteResponse =
  | { readonly ok: true }
  | {
      readonly ok: false
      readonly error: {
        /** `LIVE` is the only refusal with a remedy: stop the run, then delete. */
        readonly code: 'LIVE' | 'NOT_FOUND' | 'INTERNAL_ERROR'
        readonly message: string
      }
    }

export interface DesktopApi {
  readonly terminal: import('./terminal.js').TerminalApi
  readonly browser?: import('./browser.js').BrowserApi
  readonly voice?: import('./voice.js').VoiceApi
  /** A voice call: ChatGPT's voice, placed by Codex (0.737). */
  readonly voiceCall?: import('./voice-call.js').VoiceCallApi
  /** What was said on each call with a teammate, kept with its conversation (0.747). */
  readonly calls?: import('./calls.js').CallsApi
  /** The keys that start Talk from any app (0.755). */
  readonly talkShortcut?: import('./talk-shortcut.js').TalkShortcutApi
  /** A link that opens Locust with a message ready (0.770): never sent, the person reads it first. */
  readonly locustLinks?: import('./locust-link.js').LocustLinkApi
  /** Scout, a teammate that keeps watch (0.756). */
  readonly scout?: import('./scout.js').ScoutApi
  readonly locustMcp?: import('./locust-mcp.js').LocustMcpApi
  /** Your phone (0.775): a page on the home network, paired by a QR code. */
  readonly phone?: import('./phone.js').PhoneApi
  /** Your teammates in chat apps (0.831 Telegram, 0.832 Discord and Slack): your own bot or app, your own chat. */
  readonly telegram?: import('./chat-apps.js').ChatAppApi
  readonly discord?: import('./chat-apps.js').ChatAppApi
  readonly slack?: import('./chat-apps.js').ChatAppApi
  /** Keys a teammate needs (0.833): by name; a value goes in once and never comes back. */
  readonly teammateKeys?: import('./teammate-keys.js').TeammateKeysApi
  /** Your own agents (0.777): any Agent Client Protocol agent, by its command line. */
  readonly acpAgents?: import('./acp-agents.js').AcpAgentsApi
  /** A `!` in the chat box, and Markdown files that become slash commands. */
  readonly ownCommands?: import('./own-commands.js').OwnCommandsApi
  readQueuedMessages(): Promise<QueuedMessagesResponse>
  writeQueuedMessages(rows: readonly SavedQueuedMessage[]): Promise<QueuedMessagesResponse>
  /**
   * Locust in a browser (0.830, shared/desk.ts): set only there, naming the computer it is the window of. The window
   * hides what means nothing in a browser -- its own window controls -- and opens links in the browser.
   */
  readonly remote?: { readonly computer: string }
  /** This computer's window, while a browser has it (?elsewhere=1): the browser's name, and the way back here. */
  deskWhere?(): Promise<{ readonly device?: string }>
  deskTakeBack?(): Promise<void>
  readonly platform: string
  /** How many things need the person now: the taskbar shows it (0.379). */
  setNeedsYouCount(count: number): void
  /** Since you were away (0.590): the last attention mark, if long enough ago to count. */
  getAwaySince(): Promise<{ readonly since?: string }>
  /** A new absence ended: the window came back after at least AWAY_MINIMUM_MS. */
  onAwaySince(listener: (since: string) => void): () => void
  /** The counts for the tray's line; null when there is nothing to say. */
  setAwaySummary(counts: AwaySummaryCounts | null): void
  /** The person saw the list: the mark moves to now and the tray line goes. */
  awaySeen(): void
  /** A long run the person started finished: said as a toast while the window is elsewhere (0.379). */
  notifyFinished(finish: { readonly title: string; readonly body: string; readonly missionId?: string }): void
  /** A finished run's toast was clicked: the conversation to open. */
  onAttentionOpenMission(listener: (missionId: string) => void): () => void
  minimize(): void
  toggleMaximize(): void
  close(): void
  getAppInfo(): Promise<AppInfo>
  /** What changed in the running build, and whether this is its first launch. */
  getChangelog(): Promise<AppChangelog>
  /**
   * Called when the "what changed" banner is actually on screen. Reading the
   * changelog used to mark the version seen, so a banner held at launch
   * (nothing connected yet) was never shown on the next launch either
   * (Fable, pass 1, finding 7).
   */
  markChangelogSeen(): Promise<void>
  readStorageReport(): Promise<StorageReportResponse>
  checkForUpdate(): Promise<AppUpdateResponse>
  installUpdate(): Promise<AppUpdateResponse>
  /** Every build, or only the one a day testers get; checks again on the new lane. */
  setUpdateLane(everyBuild: boolean): Promise<AppUpdateResponse>
  /** What Windows holds for sign-in. Read again each time Settings opens. */
  readLoginItem(): Promise<LoginItemState>
  setLoginItem(openAtLogin: boolean): Promise<LoginItemState>
  readKeepRunning(): Promise<KeepRunningState>
  setKeepRunning(keepRunning: boolean): Promise<KeepRunningState>
  /** The theme this window was started in, read from its own command line: no flash on the way in (0.789). */
  readonly initialTheme: 'dark' | 'light'
  readAppearance(): Promise<AppearanceState>
  /** Dark, Light, or Match the computer; answers with what the windows are now drawn in. */
  setTheme(theme: ThemeChoice): Promise<AppearanceState>
  /** The theme changed: by the person, in another window, or by the computer under Match the computer. */
  onAppearance(listener: (state: AppearanceState) => void): () => void
  onUpdateState(listener: (state: AppUpdateState) => void): () => void
  pruneMissions(request: MissionPruneRequest): Promise<MissionPruneResponse>
  /** A folder for a backup to go in, or a backup to restore; undefined when the picker was cancelled. */
  pickProfileFolder(purpose: 'backup' | 'restore'): Promise<string | undefined>
  /** A new backup folder inside `folder`. */
  backupProfile(folder: string): Promise<ProfileBackupResponse>
  /** What restoring `folder` would bring and replace, read by the restore's own reader. */
  previewProfileRestore(folder: string): Promise<ProfileRestorePreview>
  /** Written down, then Locust restarts; the restore is applied before anything is read. */
  restoreProfile(folder: string): Promise<ProfileRestoreResponse>
  /** What the last restore did, once. */
  takeLastProfileRestore(): Promise<ProfileRestoreOutcome | undefined>
  /**
   * Ask the machine what it has. `fresh` drops every cached answer first --
   * including whether npm is there, which is otherwise decided once per
   * session. That is the Check again path: the one repair the app offers
   * should be able to repair the npm reading too (Fable, pass 2, finding 3b).
   *
   * `only` asks just those runtimes again and keeps every other answer: the
   * window's re-asks name the runtimes they are waiting on.
   */
  getLocalRuntimes(fresh?: boolean, only?: readonly string[]): Promise<RuntimeDiscoveryResponse>
  /**
   * The mission history. `known` is what this window already holds, as
   * `missionId -> digest`: a whole record that has not changed since comes
   * back as `eventsKept` with no events, so an unchanged history is not sent
   * again on every run end.
   */
  getMissionHistory(known?: Readonly<Record<string, string>>): Promise<MissionHistoryResponse>
  /** Remove a finished mission's record for good. Refused while it is live. */
  deleteMission(missionId: string): Promise<MissionDeleteResponse>
  /** What is in the trash, newest deletion first. */
  listTrashedMissions(): Promise<TrashListResponse>
  /** Put a deleted conversation back, whole. */
  restoreMission(missionId: string): Promise<TrashMutationResponse>
  /** Delete the trash for good. */
  emptyTrash(): Promise<TrashMutationResponse>
  listTeammates(): Promise<TeammateListResponse>
  /** The pets on this computer (0.563). */
  listPets(): Promise<PetListResponse>
  /** One pet's sheet, to draw. */
  readPetSheet(source: PetSource, id: string): Promise<PetSheetResponse>
  petThumbnail(id: string): Promise<PetThumbnailResponse>
  /** Download one of the pets Locust offers to this computer. Only ever on the person's click. */
  addPet(id: string): Promise<PetAddResponse>
  removePet(id: string): Promise<PetRemoveResponse>
  createTeammate(request: TeammateCreateRequest): Promise<TeammateMutationResponse>
  updateTeammate(request: TeammateUpdateRequest): Promise<TeammateMutationResponse>
  teammateSpend(): Promise<TeammateSpendResponse>
  listOwnModels(): Promise<OwnModelListResponse>
  addOwnModel(request: OwnModelAddRequest): Promise<OwnModelMutationResponse>
  removeOwnModel(ownId: string): Promise<OwnModelMutationResponse>
  testOwnModel(request: OwnModelTestRequest): Promise<OwnModelTestResponse>
  setOwnModelChatOnly(ownId: string, chatOnly: boolean): Promise<OwnModelMutationResponse>
  removeTeammate(teammateId: string): Promise<TeammateMutationResponse>
  assignMission(teammateId: string, missionId: string): Promise<TeammateMutationResponse>
  /** Name a conversation. An empty name clears it back to what was typed. */
  renameMission(missionId: string, title: string): Promise<TeammateMutationResponse>
  /** Pin a conversation (its key) to the top of the sidebar, or unpin it (0.729). */
  pinMission(missionId: string, pinned: boolean): Promise<TeammateMutationResponse>
  /** Settle (no `until`), snooze until a time, or bring back (`undefined`) a conversation (0.730). */
  settleMission(missionId: string, settled: { readonly until?: string } | undefined): Promise<TeammateMutationResponse>
  listGroups(): Promise<GroupListResponse>
  createGroup(name: string): Promise<GroupMutationResponse>
  renameGroup(groupId: string, name: string): Promise<GroupMutationResponse>
  removeGroup(groupId: string): Promise<GroupMutationResponse>
  /** Pass no group id to take a conversation out of its group. */
  assignGroup(missionId: string, groupId: string | undefined): Promise<GroupMutationResponse>
  setGroupInstructions(groupId: string, instructions: string): Promise<GroupMutationResponse>
  /** Pass no route to clear the group's default. */
  setGroupRoute(groupId: string, route: TeammateRoute | undefined): Promise<GroupMutationResponse>
  listRoutines(): Promise<RoutineListResponse>
  createRoutine(request: RoutineCreateRequest): Promise<RoutineMutationResponse>
  updateRoutine(request: RoutineUpdateRequest): Promise<RoutineMutationResponse>
  readRoutineHook(routineId: string): Promise<import('./routine-hook.js').RoutineHookResponse>
  newRoutineHook(routineId: string): Promise<import('./routine-hook.js').RoutineHookResponse>
  removeRoutine(routineId: string): Promise<RoutineMutationResponse>
  /** Pause or resume a routine's schedule (0.705). Run still runs a paused one. */
  pauseRoutine(routineId: string, paused: boolean): Promise<RoutineMutationResponse>
  /** Replay a routine: its first step starts now, each later step when the one before completes. */
  runRoutine(routineId: string, values?: Readonly<Record<string, string>>): Promise<RoutineRunResponse>
  /** Pick a folder for a routine input: the only source a folder value is accepted from (W7). */
  chooseRoutineFolder(): Promise<RoutineFolderResponse>
  /** Write a routine as a `.locust-routine.json` file (W7). */
  exportRoutine(request: RoutineExportRequest): Promise<RoutineExportResponse>
  /** Choose a routine file and read it. Nothing is created and nothing runs (W7). */
  previewRoutineImport(): Promise<RoutineImportPreviewResponse>
  /** The starter routines Locust ships (0.615). */
  listRoutineTemplates(): Promise<RoutineTemplatesResponse>
  /** One of them, previewed as an imported file is, for the same Import. */
  previewRoutineTemplate(id: string): Promise<RoutineImportPreviewResponse>
  /** Add the previewed routine for a teammate: no schedule, nothing started (W7). */
  importRoutine(request: RoutineImportRequest): Promise<RoutineMutationResponse>
  /** Keep, discard or open a copy routine's waiting changes (0.533). */
  settleRoutine(request: RoutineSettleRequest): Promise<RoutineSettleResponse>
  /** Answer a pending approval. Unknown or already-answered ids are ignored. */
  listModels(): Promise<ModelCatalogResponse>
  /** Read-only: what the installed CLIs already have set up. */
  listRuntimeArtifacts(): Promise<readonly PublicRuntimeArtifact[]>
  /** Run the install the screen showed, for a runtime that comes from npm. */
  installRuntime(runtime: string): Promise<RuntimeInstallResponse>
  /** npm output while an install runs. Returns the unsubscribe. */
  onRuntimeInstallProgress(listener: (progress: RuntimeInstallProgress) => void): () => void
  /** The coding agents Locust keeps current, and what it has done about them. */
  readRuntimeUpdates(): Promise<RuntimeUpdatesState>
  /** Update them without being asked, or not. */
  setRuntimeUpdates(automatic: boolean): Promise<RuntimeUpdatesState>
  /** Update this one now: the person pressed Update on its row. */
  updateRuntimeNow(runtime: string): Promise<RuntimeUpdatesState>
  /** Undo a turn (0.674): each run's way back, by run id. */
  turnUndoStates(runIds: readonly string[]): Promise<Readonly<Record<string, TurnUndoState>>>
  /** Puts back what the run changed, where the files are still as it left them. */
  undoTurn(runId: string): Promise<TurnUndoState>
  /** Commit (0.680): what the open folder has uncommitted, and whether it can be pushed or proposed. */
  folderChanges(): Promise<FolderChanges>
  /** Whether the open folder is a git repository's top; undefined when git cannot say (2026-10-16). */
  folderIsRepository(): Promise<boolean | undefined>
  /** The pull request for the branch the open folder is on, read with the person's own gh (0.720). */
  folderPullRequest(): Promise<FolderPullRequest | undefined>
  /** A pull request linked in a conversation: owner, repo and number, read with the person's own gh. */
  pullLink(request: PullLinkAddress): Promise<LinkedPullRequest | undefined>
  /** An issue linked in a conversation: owner, repo and number, read with the person's own gh. */
  issueLink(request: PullLinkAddress): Promise<LinkedIssue | undefined>
  /** Watch the folder's pull request, or stop (0.731); answers whether it is watched now. */
  watchPullRequest(url: string, on: boolean): Promise<boolean>
  /** The folder's changes against its base, or one of its branch's commits (0.732). */
  folderDiff(sha?: string): Promise<FolderDiff>
  /** What changed about the watched pull request (0.731). */
  onPullRequestNews(listener: (news: PullRequestNews) => void): () => void
  /** Commit it all as the person, then push or open a pull request when asked. */
  commitFolder(message: string, then: CommitThen, shown: readonly string[], fixes?: number): Promise<CommitResult>
  /** Background turns (W10): Claude Code runs them, Locust watches. */
  backgroundRuns(): Promise<readonly PublicBackgroundRun[]>
  startBackground(request: BackgroundStartRequest): Promise<BackgroundStartResponse>
  stopBackground(id: string): Promise<boolean>
  /** `claude attach <id>`, in a terminal: where a waiting run is answered. */
  openBackground(id: string): Promise<{ readonly ok: boolean; readonly message?: string }>
  /** Claude Code in the folder, in a terminal, for the person to answer its trust question once. */
  setUpBackground(): Promise<{ readonly ok: boolean; readonly message?: string }>
  dismissBackground(id: string): Promise<void>
  /** An update started, landed or failed. Returns the unsubscribe. */
  onRuntimeUpdates(listener: (state: RuntimeUpdatesState) => void): () => void
  /** Open the runtime's sign-in in its own window. */
  /** `again`: sign in once more, as this account or another (Colin, 2026-10-02). */
  agentAccounts(action: AccountAction): Promise<AgentAccountsState>
  signInRuntime(runtime: string, again?: boolean): Promise<RuntimeSignInResponse>
  /** A sign-in window Locust opened has closed, finished or not (0.545). */
  onSignInClosed(listener: (runtime: string) => void): () => void
  /** Open this conversation's session in its runtime's own terminal (0.387). */
  openInTerminal(missionId: string): Promise<OpenInTerminalResponse>
  /** Bring what was done in the terminal on this conversation into it (0.391). */
  catchUpTerminal(missionId: string): Promise<TerminalCatchUpResponse>
  /** Sessions a person had in Claude Code or Codex, to bring in (session-import.ts). */
  listImportableSessions(): Promise<SessionImportListResponse>
  importSession(runtime: ImportableSessionView['runtime'], sessionId: string): Promise<SessionImportResponse>
  saveTeamCard(rect: TeamCardRect): Promise<TeamCardSaveResponse>
  /** Save a conversation's record as one Markdown file (mission-export.ts). */
  saveMissionRecord(request: MissionRecordSaveRequest): Promise<MissionRecordSaveResponse>
  addTeamFromCard(): Promise<TeamCardAddResponse>
  readWorkspaceSettings(): Promise<WorkspaceSettings>
  /** Pick the folder the teammates work in. Reopens the app there on success. */
  chooseWorkspace(): Promise<WorkspaceChooseResponse>
  /** Start here on the question about a folder like home or `.claude` (0.790): true when the folder was kept. */
  confirmWorkspace(): Promise<boolean>
  listFolders(): Promise<FolderListResponse>
  /** `of` is the conversation's (or the side chat's) latest turn; `question` counts from 1. */
  askOnTheSide(of: string, question: string, count: number): Promise<SideAskResponse>
  draftTune(of: string): Promise<SideAskResponse>
  /** `measured` (0.811): each setup's record over every run, when its tune was run again before one was kept. */
  noteTuneLesson(compareId: string, usedInstructions?: boolean, measured?: readonly import('./tune.js').TuneMeasured[]): Promise<import('./tune.js').TuneLessonNoteResponse>
  /** One side turn writes one set of instructions with both of a kept tune's edits (0.811); nothing is saved by it. */
  combineTuneEdits(compareId: string): Promise<import('./tune.js').TuneCombineResponse>
  listTuneLessons(teammateId?: string): Promise<import('./tune.js').TuneLessonListResponse>
  removeTuneLesson(lessonId: string): Promise<import('./tune.js').TuneLessonListResponse>
  /** A teammate's own skills (0.811), every teammate's: kept from tunes, listed in Edit teammate. */
  listTeammateSkills(): Promise<import('./tune.js').TeammateSkillListResponse>
  /** Use a kept tune's skill for its teammate: `kept` is the kept setup's, `other` the one a setup not kept added. */
  useTuneSkill(compareId: string, which: 'kept' | 'other'): Promise<import('./tune.js').TeammateSkillListResponse>
  removeTeammateSkill(teammateId: string, name: string): Promise<import('./tune.js').TeammateSkillListResponse>
  /** What shapes a teammate's turns (0.811); `missionId`, its open conversation's latest turn, when it is the teammate's. */
  readTeammateSetup(teammateId: string, missionId?: string): Promise<import('./teammate-setup.js').TeammateSetupResponse>
  switchFolder(id: string): Promise<FolderSwitchResponse>
  /** Cloud tasks (0.503): this folder's standing, start, follow, show, apply. */
  cloudWhere(): Promise<PublicCloudWhere>
  startCloudTask(prompt: string, teammateId?: string): Promise<CloudStartResponse>
  listCloudTasks(): Promise<readonly PublicCloudTask[]>
  refreshCloudTask(taskId: string): Promise<PublicCloudTask | undefined>
  cloudTaskDiff(taskId: string): Promise<string | undefined>
  applyCloudTask(taskId: string): Promise<CloudApplyResponse>
  cloudFolders(): Promise<readonly PublicCloudFolder[]>
  /** Your cloud computer (0.761). */
  cloudTurns(missionIds: readonly string[]): Promise<readonly PublicCloudTurn[]>
  cloudTurnPatch(missionId: string): Promise<string | undefined>
  applyCloudTurn(missionId: string): Promise<CloudTurnResponse>
  discardCloudTurn(missionId: string): Promise<CloudTurnResponse>
  /** The change waiting there, sent to a pull request on GitHub (0.762); `added`: to the conversation's open one. */
  pullRequestCloudTurn(missionId: string, message: string): Promise<CloudPullRequestResponse>
  cloudComputers(): Promise<CloudComputersResponse>
  stopCloudComputer(name: string): Promise<{ readonly ok: true } | { readonly ok: false; readonly message: string }>
  removeCloudComputer(name: string): Promise<{ readonly ok: true } | { readonly ok: false; readonly message: string }>
  openCloudComputerTerminal(name: string): Promise<OpenInTerminalResponse>
  /** GitHub's codespace permission for this computer's gh, given in a terminal Locust opens (gh auth refresh). */
  allowCloudComputers(): Promise<OpenInTerminalResponse>
  /** This folder's repository and its cloud computer, if it has one. */
  cloudComputerForFolder(): Promise<CloudFolderState>
  /**
   * Made and readied for this folder's repository, each step said as a 'cloud-setup' update; for a Claude Code or
   * Codex teammate (0.768), that agent on it, signed in in the person's own browser.
   */
  setUpCloudComputer(agent?: CloudAgent): Promise<CloudFolderState>
  /** Claude Code or Codex signed in on that cloud computer, in the person's own browser (0.768): 'cloud-sign-in' updates. */
  signInCloudAgent(name: string, agent: CloudAgent): Promise<CloudSignInResponse>
  /** The sign-in under way, stopped. */
  stopCloudSignIn(): Promise<void>
  signOutCloudAgent(name: string, agent: CloudAgent): Promise<{ readonly ok: true } | { readonly ok: false; readonly message: string }>
  /** Claude's cloud (0.538): a window of Claude Code with the task given; the list; `--teleport` to bring one home. */
  /** `choice`: the box's Claude model and effort, which the session runs on (0.557). */
  startClaudeCloud(prompt: string, teammateId?: string, choice?: { readonly model?: string; readonly effort?: string; readonly environment?: string }): Promise<ClaudeCloudStartResponse>
  getRemoteControl(): Promise<RemoteControlState>
  setRemoteControl(enabled: boolean): Promise<RemoteControlState>
  getClaudeCloudEnvironment(): Promise<string | undefined>
  listClaudeCloud(): Promise<readonly PublicClaudeCloudSession[]>
  bringClaudeCloudHome(id: string): Promise<ClaudeCloudHomeResponse>
  continueClaudeCloudInTerminal(id: string): Promise<OpenInTerminalResponse>
  forgetClaudeCloud(id: string): Promise<void>
  /** A follow-up to a cloud session Locust knows the id of (0.556). */
  sendClaudeCloud(id: string, message: string): Promise<ClaudeCloudHomeResponse>
  checkClaudeCloud(id: string): Promise<ClaudeCloudReading>
  applyClaudeCloud(id: string): Promise<ClaudeCloudHomeResponse>
  /** Undo the later turns' file changes before an edited message goes (0.502). */
  putBackFiles(request: RewindPutBackRequest): Promise<RewindPutBackResponse>
  /**
   * Show a file in the file manager. Answers whether it was shown, so the
   * card can say something rather than appear to do nothing.
   */
  revealFile(path: string): Promise<RevealFileResponse>
  /** Save a copy of a file a teammate wrote, to a place the person picks. */
  saveCopy(path: string): Promise<RevealFileResponse>
  /** Read a workspace text file for the viewer. */
  readTextFile(path: string): Promise<WorkspaceTextResponse>
  /** Where a web page in the folder is served for the preview frame (0.425). */
  /** `column`: a comparison column whose copy a relative path is read from first (0.452). */
  pageUrlFor(path: string, column?: { readonly compareId: string; readonly slot: string }): Promise<WorkspacePageResponse>
  /** The address a reply's whole web page runs at (0.553). */
  replyPageUrl(html: string): Promise<WorkspacePageResponse>
  /** A commit message written from the changes by the teammate of conversation `of` (0.771, shared/commit-draft.ts). */
  draftCommitMessage(of: string, asks: readonly string[]): Promise<import('./commit-draft.js').CommitDraftResponse>
  /** A walk through the change the Changes panel shows, by the teammate of conversation `of` (0.771, shared/walkthrough.ts). */
  walkThroughChange(of: string, asks: readonly string[], sha?: string): Promise<import('./walkthrough.js').WalkthroughResponse>
  /** The runs whose answer belongs to a panel (0.771): never listed as conversations. */
  panelRuns(): Promise<readonly string[]>
  /** A reply's Mermaid diagram as a picture, drawn in a view no one sees (0.770, shared/diagram.ts). */
  drawDiagram(source: string): Promise<DiagramResponse>
  /** Each runtime's own slash commands, as its CLI last listed them (0.426). */
  runtimeCommands(): Promise<RuntimeCommandsResponse>
  /** Show the diagnostics log in the file manager. Names no path. */
  revealDiagnostics(): Promise<void>
  /** Open the folder teammates work in, in the file manager. Names no path: the host opens its own. */
  openWorkspaceFolder(): Promise<{ readonly ok: true } | { readonly ok: false; readonly message: string }>
  /** Open the feedback report, filled in, in the person's browser. Names no address. */
  sendFeedback(report: FeedbackReport): Promise<OpenLinkResponse>
  /** The same report, as an email to the support address (0.593): opens the mail app, filled in. */
  emailFeedback(report: FeedbackReport): Promise<OpenLinkResponse>
  /** The same report, whole, saved where the person chooses (0.593). `path` when saved; a cancel is ok with no path. */
  saveFeedbackFile(report: FeedbackReport): Promise<{ readonly ok: boolean; readonly path?: string; readonly message?: string }>
  /** Where the log is, for the sentence that tells a person what to send. */
  diagnosticsReport(): Promise<DiagnosticsReport>
  /** Open one of the addresses the host allows, in the person's browser. */
  openLink(url: string): Promise<OpenLinkResponse>
  /** On a Mac: a newer release with this chip's disk image, when there is one (0.515). */
  macRelease(): Promise<MacReleaseAnswer | undefined>
  /** What a reply on another runtime would carry (0.517); undefined when it could not be worked out. */
  previewHandoff(request: HandoffPreviewRequest): Promise<HandoffPreview | undefined>
  /** Open the picker for files to attach; answers workspace-relative paths. */
  attachFiles(): Promise<AttachFilesResponse>
  /** The project's files, workspace-relative, for `@` in the composer (0.436). */
  workspaceFiles(): Promise<WorkspaceFilesResponse>
  /** Usage across every agent and model over a range (0.714). */
  readUsage(range: UsageRange): Promise<UsageReadResponse>
  /** An attached PDF's page pictures, folder-relative (0.714); read on the way when it has none yet. */
  pdfPages(path: string, folder?: string): Promise<PdfPagesResponse>
  /** One connector added to every agent chosen, each through its own `mcp add` (0.716). */
  addConnector(request: ConnectorAddRequest): Promise<ConnectorAddResponse>
  /** Undo an add: each agent's own `mcp remove` (0.716). */
  removeConnector(name: string, agents: readonly ConnectorAgent[]): Promise<ConnectorAddResponse>
  /** Who the GitHub CLI is signed in as, if anyone (0.720). */
  githubAccount(): Promise<GithubAccount>
  /** `gh auth login --web`: answers when GitHub says yes, or it stops; the code arrives through `onGithubSignInCode`. */
  githubSignIn(): Promise<GithubSignInResult>
  githubSignInCancel(): Promise<void>
  /** Install the GitHub CLI with winget, or Homebrew on a Mac (0.724); then who it is signed in as. */
  githubInstall(): Promise<GithubInstallResult>
  /** Update the GitHub CLI the same way (0.726). */
  githubUpdate(): Promise<GithubInstallResult>
  /** Which GitHub CLI this is, and the newest GitHub has released (0.726). */
  githubCliVersions(): Promise<GithubCliVersions>
  onGithubSignInCode(listener: (code: GithubSignInCode) => void): () => void
  /**
   * Attach one thing from the clipboard. `bytes` is the file's contents; the
   * name is a suggestion the host sanitises and may change to avoid a
   * collision.
   */
  attachPasted(name: string, bytes: Uint8Array): Promise<AttachFilesResponse>
  /** This folder's open GitHub issues, through the person's own gh. The window never names a folder or a URL. */
  githubIssues: {
    ready(): Promise<boolean>
    list(): Promise<readonly ListedIssue[]>
    open(number: number): Promise<{ readonly brief: string; readonly issue: StartedIssue; readonly images: readonly { readonly name: string; readonly base64: string }[] } | undefined>
    remembered(missionId: string): Promise<StartedIssue | undefined>
  }
  /** Point at a part of the running page; answers when it is clicked, or cancelled (0.484). */
  pickInPage(request: PagePickRequest): Promise<PagePickResponse>
  cancelPagePick(pageUrl: string): Promise<void>
  readWorkspaceImage(path: string, folder?: string): Promise<WorkspaceImageResponse>
  listRooms(): Promise<RoomListResponse>
  createRoom(request: RoomCreateRequest): Promise<RoomMutationResponse>
  removeRoom(roomId: string): Promise<RoomMutationResponse>
  /** Give a room a name. A room made from an ask starts as `Untitled room`. */
  renameRoom(roomId: string, name: string): Promise<RoomMutationResponse>
  postToRoom(request: RoomPostRequest): Promise<RoomPostResponse>
  /** Send a message to the teammates tagged in it, each in a conversation of their own (0.438). */
  tagTeammates(request: TagTeammatesRequest): Promise<TagTeammatesResponse>
  /** Compare (0.441): start one, ask every column again, keep one, list them. */
  startCompare(request: CompareStartRequest): Promise<CompareResponse>
  askCompare(compareId: string, prompt: string): Promise<CompareResponse>
  keepCompare(compareId: string, slot: CompareSlotId): Promise<CompareResponse>
  retryCompare(compareId: string, slot: CompareSlotId): Promise<CompareResponse>
  /** Ask a judge for its view of a comparison's answers (0.520). */
  judgeCompare(request: CompareJudgeRequest): Promise<CompareResponse>
  /** Put the comparison's question to one more model (0.523). */
  addCompareModel(request: CompareAddModelRequest): Promise<CompareResponse>
  /** Your own benchmark (0.763): it runs again once or three times more, unattended, each run scored by its judge. */
  rerunCompare(compareId: string, times: number, waitForChecks?: boolean): Promise<CompareRerunResponse>
  checkedCompareRerun(originId: string, compareId: string): Promise<boolean>
  draftJudgePoints(request: CompareAddModelRequest): Promise<CompareDraftPointsResponse>
  stopCompareRerun(compareId: string): Promise<void>
  compareReruns(): Promise<readonly RerunProgress[]>
  compareChanges(compareId: string): Promise<CompareChangesResponse>
  compareChangesRefusal(): Promise<string | undefined>
  compareWorksInPlace(): Promise<boolean>
  listCompares(): Promise<CompareListResponse>
  updateRoomTask(request: RoomTaskRequest): Promise<RoomTaskResponse>
  listMemories(): Promise<MemoryListResponse>
  /** Each runtime's own MCP servers and hooks, read-only. */
  readRuntimeSetup(): Promise<RuntimeSetupResponse>
  /**
   * Choose the folder one teammate works in, or clear it back to the project
   * folder. Nothing reopens: only that teammate's next run moves.
   */
  chooseTeammateFolder(teammateId: string, clear?: boolean): Promise<TeammateFolderResponse>
  /** Every connector the person's Claude Code reports. Cached by the host; slow to refresh. */
  listConnectors(): Promise<ConnectorListResponse>
  /**
   * Which connectors one teammate may use without asking. The WHOLE list every
   * time; an empty list means every connector, which is the default.
   */
  setTeammateConnectors(teammateId: string, names: readonly string[]): Promise<TeammateFolderResponse>
  /** The teammates' own worktrees under the folder, and whether the folder can have them. */
  listWorktrees(): Promise<WorktreeListResponse>
  /**
   * Remove a teammate's worktree. The branch stays. Refused while a run is
   * live in it, and while it has uncommitted changes unless `discard` names
   * exactly those (C1).
   */
  removeWorktree(teammateId: string, discard?: readonly string[]): Promise<WorktreeListResponse>
  /** Review changes on a teammate's own branch (0.439). Reads only. */
  reviewBranch(teammateId: string): Promise<BranchReviewResponse>
  turnDiff(teammateId: string, sha: string): Promise<TurnDiffResponse>
  /** Land it (0.440). */
  landPreview(teammateId: string): Promise<LandPreviewResponse>
  landBranch(teammateId: string, message: string): Promise<LandResponse>
  startResolving(teammateId: string): Promise<ResolveResponse>
  /**
   * H3: one mission's record with its events. History sends the newest
   * missions whole and every other one as a row; opening one of those reads
   * it here.
   */
  readMission(missionId: string): Promise<MissionReadResponse>
  readContextBreakdown?(missionId: string): Promise<import('./context-breakdown.js').ClaudeContextBreakdown | undefined>
  references?(request: import('./references.js').ReferenceRequest): Promise<import('./references.js').ReferencesReply>
  /** A turn that ran out of its agent's plan, carried on with `route` (0.774, shared/carry-on.ts): the card's one press. */
  carryOn(missionId: string, route: CarryOnRoute): Promise<CarryOnResponse>
  addMemory(request: MemoryAddRequest): Promise<MemoryListResponse>
  updateMemory(request: MemoryUpdateRequest): Promise<MemoryListResponse>
  removeMemory(memoryId: string): Promise<MemoryListResponse>
  clearMemories(request: MemoryClearRequest): Promise<MemoryListResponse>
  /** Put a recently forgotten memory back, as it was. */
  restoreMemory(memoryId: string): Promise<MemoryListResponse>
  /** Skills from a GitHub repository (0.710): what a link holds, before anything is kept. */
  previewSkillLibrary(link: string): Promise<SkillLibraryPreviewResponse>
  /** Keep the named skills of a previewed repository, at the commit the preview read. */
  installSkillLibrary(request: SkillLibraryInstallRequest): Promise<SkillLibraryListResponse>
  listSkillLibrary(): Promise<SkillLibraryListResponse>
  /** Remove every skill kept from one repository (`owner/repo`). */
  removeSkillLibrary(source: string): Promise<SkillLibraryListResponse>
  writeWorkspaceSettings(settings: WorkspaceSettings): Promise<WorkspaceSettings>
  decideMissionApproval(answer: MissionApprovalAnswer): Promise<{ readonly ok: boolean }>
  /** Saved approval rules (0.521). */
  listApprovalRules(): Promise<ApprovalRulesResponse>
  removeApprovalRule(ruleId: string): Promise<ApprovalRulesResponse>
  removeAllApprovalRules(): Promise<ApprovalRulesResponse>
  /** Answer this card, and save its rule so the same is not asked again. */
  ruleFromApprovalCard(request: ApprovalRuleFromCardRequest): Promise<ApprovalRulesResponse>
  onMissionApproval(listener: (request: MissionApprovalRequest) => void): () => void
  onMissionApprovalWithdrawn(listener: (approvalId: string) => void): () => void
  startCodexMission(request: CodexMissionStartRequest): Promise<CodexMissionStartResponse>
  cancelCodexMission(request: CodexMissionCancelRequest): Promise<CodexMissionCancelResponse>
  handOffMission(request: MissionHandoffRequest): Promise<MissionHandoffResponse>
  resumeMission(request: MissionResumeRequest): Promise<MissionHandoffResponse>
  onCodexMissionUpdate(listener: (update: CodexMissionUpdate) => void): () => void
  /**
   * Discovery, as it happens. Replays whatever has already been emitted
   * before this launch's sweep, so a renderer that subscribes late still
   * sees the whole log rather than joining halfway.
   */
  onDiscoveryEvent(listener: (event: DiscoveryEvent) => void): () => void
  /**
   * Everything discovery has said so far this launch.
   *
   * Pushing alone is not enough and this was measured: the host replays its
   * log the moment the window is created, and the renderer has not mounted
   * yet, so those messages arrive at a page with no listener and are gone.
   * Push carries what happens next; this carries what already happened.
   */
  discoveryLog(): Promise<readonly DiscoveryEvent[]>
  /**
   * Said by the loading window when its sequence has finished.
   *
   * The app window is built but not shown until this arrives, so the
   * runtimes are already answered by the time anybody sees the workspace --
   * which is the whole point of a loading screen (Colin, 2026-09-14: "have
   * just that monitor screen be the loading screen and once its done, THEN
   * go to our app").
   */
  splashDone(): void
  recoverRoutine(request: import('./routine-recovery.js').RoutineRecoveryRequest): Promise<import('./routine-recovery.js').RoutineRecoveryResponse>
}

/** The words for what a teammate does: their own for a Custom role, the role's name otherwise. */
/**
 * What each role is for, in a few words: the role picker's second line, and
 * the line under an idle teammate's name. That line said "reads this
 * workspace and explains what it finds" for every role -- a coder, a
 * bookkeeper and a writer alike (the first-session drive, 0.358).
 */
export const ROLE_DESCRIPTIONS: Readonly<Record<TeammateRole, string>> = {
  'Code & Migrations': 'Repo work, refactors, test runs',
  'Research & Briefs': 'Reading, comparing, summarising',
  'Ops & Scheduling': 'Routine jobs and reminders',
  'Docs & QA': 'Written output and checking',
  'Data & Reporting': 'Spreadsheets, figures, digests',
  'Chief of Staff': 'Routes work to the team, reports back',
  Custom: 'Describe the work yourself'
}

export function roleLabelOf(teammate: Pick<PublicTeammate, 'role' | 'roleTitle'>): string {
  const title = teammate.roleTitle?.trim() ?? ''
  return teammate.role === 'Custom' && title.length > 0 ? title : teammate.role
}
