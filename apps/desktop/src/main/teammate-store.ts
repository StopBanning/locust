import { isAccountId } from '../shared/agent-accounts.js'
import { parsedCheckFixes } from '../shared/check-fix.js'
import { randomUUID } from 'node:crypto'
import { constants as fsConstants } from 'node:fs'
import { mkdir, open, readFile, unlink } from 'node:fs/promises'
import { isAbsolute, join, win32, posix } from 'node:path'

import { cleanAvatar, isAvatarSpec, seedAvatar } from '../shared/avatar.js'
import { MAX_TEAMMATE_INSTRUCTIONS } from '../shared/ipc.js'
import { instructionsText } from '../shared/teammate-instructions.js'
import type { PublicTeammate, TeammateHue, TeammateRole, TeammateRoute, WorkspaceSettings, MemoryMode, LayoutPreference, TubePreference, ReplyTextSize } from '../shared/ipc.js'
import { DEFAULT_RELAY_HOP_CAP, MAX_RELAY_HOP_CAP, MIN_RELAY_HOP_CAP, DEFAULT_MEMORY_MODE } from '../shared/ipc.js'
import { isMissionRuntime } from '../shared/runtimes.js'
import { isMonthlyLimit } from '../shared/spend.js'
import { isChatGptVoice } from '../shared/voice-call.js'
import { parsedAboutYou, parsedAboutYouSuggestions } from '../shared/about-you.js'
import { parsedPlanRunsOut } from '../shared/carry-on.js'
import { aboutYouSecretRefusal, secretIn } from '../shared/secrets.js'
import { replaceFile } from './replace-file.js'
import { OrganizationPolicyError } from './organization-policy.js'
import { policyMonthlyLimit, policyModeAllowed, policyRefusal, policyTurnRefusal, type OrganizationPolicyState } from '../shared/organization-policy.js'

/**
 * Teammates are local identity plus routing defaults: a name, an avatar hue, a
 * role, and which route and approval mode their missions start on. Nothing here
 * runs anything -- the multi-agent runtime is a separate problem, and the
 * roster does not need it to be real.
 *
 * The file is treated as untrusted on read. It lives in the user's profile
 * where anything can edit it, and a malformed record must degrade to "that
 * teammate is missing" rather than to a renderer crash or, worse, a teammate
 * whose route says something the runtime layer never validated.
 */

export const MAX_TEAMMATES = 64
/**
 * How many connectors one teammate may be narrowed to. Each becomes an allow
 * rule on a real command line; sixteen is above the ten a live account had
 * on 2026-09-09 and far below anything that would trouble a command line.
 */
export const MAX_CONNECTORS = 16

/**
 * How many mission-to-teammate assignments the roster will hold.
 *
 * Teammates were capped and assignments were not, and the file has a size
 * cliff: past `MAX_FILE_BYTES` it USED to read as EMPTY, and the next
 * write saved that empty file over the real one. A window that assigned
 * missions in a loop could therefore delete the person's whole roster. (It
 * reads as UNREADABLE now, and no write happens; the cap stays because a
 * roster that stops working is still a roster that stops working.)
 *
 * The number has to sit well UNDER that cliff to be worth anything. An entry
 * is roughly 65 bytes (a `mission_<uuid>` key and a teammate id), so five
 * thousand is about a third of a megabyte against a one-megabyte limit --
 * headroom enough for the teammates and settings beside it. My first attempt
 * at this was 20,000, and the test written for it proved that a full roster
 * crossed the cliff and emptied itself, which is the bug the cap exists to
 * prevent. Five thousand is still more than a dozen missions a day for a year.
 */
export const MAX_MISSION_OWNERS = 5_000

/**
 * How many conversations may carry a name a person typed, and how long one
 * may be.
 *
 * Bounded for the same reason ownership is: this file has a size cliff past
 * which it reads as EMPTY, and the next write saves that empty file over the
 * real one. A title is far bigger than an assignment -- a whole sentence
 * rather than two ids -- so it gets a tighter count and a hard length.
 */
export const MAX_MISSION_TITLES = 1_000
export const MAX_MISSION_TITLE_LENGTH = 120
/** Pinned conversations a sidebar keeps at its top (0.729): enough for any list, few enough to read. */
export const MAX_MISSION_PINS = 50
/** Settled or snoozed conversations kept out of the sidebar (0.730). */
export const MAX_MISSION_SETTLED = 1_000

/** A conversation put out of the way: when, and until when for a snooze (both ISO). */
export interface MissionSettled {
  readonly at: string
  readonly until?: string
}
const MAX_FILE_BYTES = 1_000_000
/**
 * What a caller sees when the roster file exists and cannot be read.
 *
 * NOT "no teammates". Every write here reads first, so a roster that read as
 * empty was one save away from being replaced by an empty one -- and the
 * save that would have done it is `assignMission`, which runs at every
 * mission start. A transient lock at the wrong moment would have been the
 * whole roster. Found by sweeping for the class on 2026-09-16, after Astra
 * found the same shape in the groups store.
 */
export const TEAMMATES_UNREADABLE = 'TEAMMATES_UNREADABLE'
const SCHEMA_VERSION = 1 as const

export const TEAMMATE_HUES: readonly TeammateHue[] = ['lime', 'blue', 'violet', 'clay', 'teal', 'butter', 'rose', 'slate', 'pearl', 'indigo', 'coral']

export const TEAMMATE_ROLES: readonly TeammateRole[] = [
  'Code & Migrations',
  'Research & Briefs',
  'Ops & Scheduling',
  'Docs & QA',
  'Data & Reporting',
  'Chief of Staff',
  'Custom'
]

export interface TeammateStore {
  list(): Promise<readonly PublicTeammate[]>
  create(input: { name: unknown; hue: unknown; role: unknown; roleTitle?: unknown; worktree?: unknown; avatar?: unknown; monthlyLimitUsd?: unknown; starters?: unknown; instructions?: unknown; voice?: unknown }): Promise<PublicTeammate>
  remove(teammateId: unknown): Promise<void>
  /**
   * Change what a person may change; the id and the missions filed under it
   * stay. `monthlyLimitUsd`: a number sets it, `null` removes it, omitted
   * keeps it.
   */
  update(input: { teammateId: unknown; name: unknown; hue: unknown; role: unknown; roleTitle?: unknown; worktree?: unknown; avatar: unknown; monthlyLimitUsd?: unknown; instructions?: unknown; voice?: unknown }): Promise<PublicTeammate>
  /** Record the route a person just started this teammate on. Unknown teammate or bad route: nothing changes. */
  rememberRoute(teammateId: unknown, route: unknown): Promise<void>
  /**
   * Point one teammate at its own folder, or `undefined` to put it back in
   * the project folder. The path comes from the host's own dialog, so this
   * refuses anything that is not an absolute path rather than storing it.
   */
  setFolder(teammateId: unknown, folder: string | undefined): Promise<PublicTeammate>
  /**
   * Narrow one teammate to these connectors, or widen it back to every one
   * with an empty list. The whole list every time, so the dialog's ticks and
   * the record cannot get out of step.
   */
  setConnectors(teammateId: unknown, names: unknown): Promise<PublicTeammate>
  /** Give one teammate its own browser sign-ins, or take them away (0.839). Only a literal true turns them on. */
  setOwnBrowser(teammateId: unknown, on: unknown): Promise<PublicTeammate>
  /** Remember which teammate a mission belongs to. */
  assignMission(teammateId: unknown, missionId: unknown): Promise<void>
  /** Forget which teammate a mission belonged to, once the mission is gone. */
  unassignMission(missionId: unknown): Promise<void>
  missionOwners(): Promise<Readonly<Record<string, string>>>
  missionTitles(): Promise<Readonly<Record<string, string>>>
  /** An empty or blank name CLEARS it, back to the words that were typed. */
  renameMission(missionId: string, title: string): Promise<void>
  /** Conversations pinned to the top of the sidebar, newest pin first, by conversation key (0.729). */
  missionPins(): Promise<readonly string[]>
  pinMission(missionId: string, pinned: boolean): Promise<void>
  /** Conversations settled or snoozed out of the sidebar, by conversation key (0.730). */
  missionSettled(): Promise<Readonly<Record<string, MissionSettled>>>
  /** Settle it (no `until`), snooze it until a time, or bring it back (`undefined`). */
  settleMission(missionId: string, settled: { readonly until?: string } | undefined): Promise<void>
  /**
   * Record the newest turn of this teammate's hub -- the conversation their
   * replies to other teammates continue. Unknown teammate or bad id: nothing
   * changes, the same shape as `rememberRoute`.
   */
  rememberHub(teammateId: unknown, missionId: unknown): Promise<void>
  readSettings(): Promise<WorkspaceSettings>
  writeSettings(settings: unknown): Promise<WorkspaceSettings>
}

interface StoredFile {
  readonly schemaVersion: typeof SCHEMA_VERSION
  readonly teammates: readonly PublicTeammate[]
  readonly missionOwners: Readonly<Record<string, string>>
  /** Names people typed for conversations, by mission id. */
  readonly missionTitles: Readonly<Record<string, string>>
  /** Conversations pinned to the top of the sidebar, newest pin first (0.729). */
  readonly missionPins: readonly string[]
  /** Conversations settled or snoozed out of the sidebar (0.730). */
  readonly missionSettled: Readonly<Record<string, MissionSettled>>
  readonly settings: WorkspaceSettings
}

// Relay is ON unless switched off: teammates talking to each other is the
// point of having more than one, and the hop cap is what bounds the spend.
const DEFAULT_SETTINGS: WorkspaceSettings = { swarm: false, relay: true, relayHopCap: DEFAULT_RELAY_HOP_CAP, interrupt: false, memoryMode: DEFAULT_MEMORY_MODE, autoMode: false, askConnectors: false, keepATodoList: true, layout: 'auto', tube: 'full', replySize: 'standard', metal: 'silver', metalStrength: 'standard', metalMotion: 'hover', metalBend: false, terminalFaces: true, plush: false, claudeOwnSkills: false }

/** A layout this build can draw, or the default. Never trusts the file. */
function parsedLayout(value: unknown): LayoutPreference {
  return value === 'compact' || value === 'wide' || value === 'auto' ? value : 'auto'
}

/** Whether a teammate keeps a plan. Absent means never chosen, so: the default. */
function parsedTodoList(value: unknown): boolean {
  return typeof value === 'boolean' ? value : true
}

/** How much of the boot screen to draw, or the default. Never trusts the file. */
/** How big the reply is set, or the default. Never trusts the file. */
/**
 * One of a known set, or the default. The same shape every other parser here
 * uses, and for the same reason: a settings file is a file on a disk, and a
 * value that is not one of the options has to read as "not set" rather than
 * reach the renderer and be rendered.
 */
function oneOf<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === 'string' && (allowed as readonly string[]).includes(value) ? (value as T) : fallback
}

const METAL_PRESETS = ['off', 'chromatic', 'silver', 'gold'] as const
const METAL_STRENGTHS = ['subtle', 'standard', 'strong'] as const
const METAL_MOTIONS = ['hover', 'always'] as const

/** Terminal faces (0.561): on unless a stored false says off -- an absent key is the new default. */
function parsedTerminalFaces(raw: unknown): boolean {
  return !(typeof raw === 'object' && raw !== null && (raw as Record<string, unknown>).terminalFaces === false)
}

/** The send button's metal, read off whatever the file happens to hold. */
function parsedMetal(raw: unknown): Pick<WorkspaceSettings, 'metal' | 'metalStrength' | 'metalMotion' | 'metalBend'> {
  const record = typeof raw === 'object' && raw !== null ? (raw as Record<string, unknown>) : {}
  return {
    metal: oneOf(record.metal, METAL_PRESETS, 'silver'),
    metalStrength: oneOf(record.metalStrength, METAL_STRENGTHS, 'standard'),
    metalMotion: oneOf(record.metalMotion, METAL_MOTIONS, 'hover'),
    /*
     * OFF unless it was switched on.
     *
     * Colin, 2026-09-21: "have cursor bend automatically off, user can turn
     * on in settings if they like." It is a flourish on the send button, and
     * a flourish is the kind of thing a new user should meet only if they
     * went looking for it.
     *
     * A stored boolean still wins, in BOTH directions: someone who turned it
     * on keeps it on, and someone who turned it off keeps it off. Only an
     * ABSENT key -- never chosen -- reads as the new default. (The reverse
     * reading is what made the todo-list default a no-op for everyone who
     * already had a settings file; see `parsedTodoList`.)
     */
    metalBend: typeof record.metalBend === 'boolean' ? record.metalBend : false
  }
}

/**
 * A3.3: one check command, as the person wrote it -- one line, bounded, or
 * nothing. It is run through the shell, so it is kept exactly as typed; what
 * is refused is what could not have been typed into one line.
 */
export function parsedCheckCommand(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined
  const command = value.trim()
  if (command.length === 0 || command.length > 500 || /[\r\n\0]/.test(command)) return undefined
  return command
}

function parsedCheckCommands(value: unknown): Readonly<Record<string, string>> | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const kept: Record<string, string> = {}
  for (const [folder, command] of Object.entries(value as Record<string, unknown>).slice(0, 200)) {
    const parsed = parsedCheckCommand(command)
    if (folder.length > 0 && folder.length <= 200 && parsed !== undefined) kept[folder] = parsed
  }
  return Object.keys(kept).length === 0 ? undefined : kept
}

function parsedReplySize(value: unknown): ReplyTextSize {
  return value === 'standard' || value === 'large' || value === 'largest' ? value : 'standard'
}

function parsedTube(value: unknown): TubePreference {
  return value === 'full' || value === 'subtle' || value === 'off' ? value : 'full'
}

function parsedMemoryMode(value: unknown): MemoryMode {
  return value === 'auto' || value === 'ask' || value === 'off' ? value : DEFAULT_MEMORY_MODE
}

/** A cap from disk or from the window: an integer inside the bounds, or the default. */
function parsedHopCap(value: unknown): number {
  return typeof value === 'number' && Number.isInteger(value) && value >= MIN_RELAY_HOP_CAP && value <= MAX_RELAY_HOP_CAP
    ? value
    : DEFAULT_RELAY_HOP_CAP
}

export function isTeammateRoute(value: unknown): value is TeammateRoute {
  if (typeof value !== 'object' || value === null) return false
  const record = value as Record<string, unknown>
  return (
    isMissionRuntime(record.runtime)
    && typeof record.model === 'string'
    && record.model.length > 0
    && record.model.length <= 200
    // `plan` is a mode like the others, so a teammate last started in it
    // records that honestly. Refusing it here would not stop the mode -- it
    // would only make `rememberRoute` fail silently and leave a stale one on
    // the roster, which is the worse of the two.
    && (record.mode === 'ask' || record.mode === 'accept-edits' || record.mode === 'approve-each' || record.mode === 'plan' || record.mode === 'auto')
    // Optional, and only ever a short string: a routine records the level it
    // was taught with. Bounded rather than free -- it is written into a command
    // line, and a route carrying something absurd should be refused here rather
    // than at the runtime.
    && (record.account === undefined || isAccountId(record.account))
    && (record.effort === undefined
      || (typeof record.effort === 'string' && record.effort.length > 0 && record.effort.length <= 40))
  )
}

function isHue(value: unknown): value is TeammateHue {
  return typeof value === 'string' && (TEAMMATE_HUES as readonly string[]).includes(value)
}

function isRole(value: unknown): value is TeammateRole {
  return typeof value === 'string' && (TEAMMATE_ROLES as readonly string[]).includes(value)
}

/**
 * A name the UI can render and the file can hold. Control characters are
 * refused rather than stripped: a name is shown back to the user, and silently
 * rewriting what they typed is worse than telling them it is invalid.
 */
/**
 * A Custom role's title: the person's own words, one line, no control
 * characters, short enough to sit beside a name. It is briefed to every
 * runtime on the roster, so it is bounded like a name is.
 */
export const MAX_ROLE_TITLE_LENGTH = 60

export function validRoleTitle(value: unknown): value is string {
  if (typeof value !== 'string') return false
  const trimmed = value.trim()
  return trimmed.length > 0 && trimmed.length <= MAX_ROLE_TITLE_LENGTH && !/[\u0000-\u001f\u007f]/.test(trimmed)
}

const MAX_STARTERS = 4
const MAX_STARTER_LENGTH = 300

/**
 * A template's first messages for a teammate (0.359): one to four lines of
 * plain text, or nothing. On disk a malformed list is dropped -- the role's
 * own are offered instead -- and never a reason to lose the teammate.
 */
function parsedStarters(value: unknown): readonly string[] | undefined {
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_STARTERS) return undefined
  const lines = value.map((line) => (typeof line === 'string' ? line.trim() : ''))
  return lines.every((line) => line.length > 0 && line.length <= MAX_STARTER_LENGTH && !/[\u0000-\u001f\u007f]/.test(line)) ? lines : undefined
}

/** Only a Custom role keeps a title, and only a valid one. Anything else is dropped, never rejected. */
/** A teammate's own instructions (0.706): trimmed text up to the cap, or nothing. */
function parsedInstructions(value: unknown): string | undefined {
  const text = instructionsText(value)
  return text === undefined || text.length === 0 ? undefined : text
}

function roleTitleFor(role: TeammateRole, value: unknown): string | undefined {
  if (role !== 'Custom' || !validRoleTitle(value)) return undefined
  return value.trim()
}

/**
 * A2.18: TWO TEAMMATES MAY NOT SHARE A NAME.
 *
 * A teammate is reached by the name a model writes in `to=`, and a name two
 * teammates answer to is refused as ambiguous (peer-exchange.ts
 * `recipientOf`): with two Wrens, a message could reach neither. Refused at
 * the only two places a name is given -- creating and editing -- ignoring
 * case and the spaces around it, the way the name is matched.
 */
export class TeammateNameTakenError extends Error {
  constructor(readonly taken: string) {
    super(`Another teammate is already called ${taken}. Pick another name.`)
  }
}

function nameTaken(teammates: readonly PublicTeammate[], name: string, except?: string): string | undefined {
  const wanted = name.trim().toLowerCase()
  return teammates.find((teammate) => teammate.teammateId !== except && teammate.name.trim().toLowerCase() === wanted)?.name
}

export function validName(value: unknown): value is string {
  if (typeof value !== 'string') return false
  const trimmed = value.trim()
  return trimmed.length > 0 && trimmed.length <= 40 && !/[\u0000-\u001f\u007f]/.test(trimmed)
}

export function safeId(value: unknown): value is string {
  return typeof value === 'string' && /^[A-Za-z0-9_-]{1,64}$/.test(value)
}

/**
 * A folder a teammate may be pointed at.
 *
 * Absolute only, and short of the path limit both platforms enforce. The
 * folder EXISTING is not checked here: a record is read at launch, and a
 * teammate whose folder is on a drive that is not mounted this morning
 * should still be a teammate -- the start says so, rather than the record
 * quietly losing the setting.
 */
export function isTeammateFolder(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0 && value.length <= 4096 && (isAbsolute(value) || win32.isAbsolute(value) || posix.isAbsolute(value))
}

/**
 * Connector names a record may carry. Read strictly: each becomes an allow
 * rule on a real command line, so a malformed record must not be able to
 * widen anything. Anything that is not a plain, bounded string is dropped.
 */
export function parsedConnectors(value: unknown): readonly string[] {
  if (!Array.isArray(value)) return []
  const kept: string[] = []
  for (const entry of value as readonly unknown[]) {
    if (typeof entry !== 'string') continue
    const name = entry.trim()
    if (name.length === 0 || name.length > 120) continue
    if (/[\u0000-\u001f]/.test(name)) continue
    if (!kept.includes(name)) kept.push(name)
    if (kept.length >= MAX_CONNECTORS) break
  }
  return kept
}

export function parsedTeammate(value: unknown): PublicTeammate | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const record = value as Record<string, unknown>
  if (
    !safeId(record.teammateId) ||
    !validName(record.name) ||
    !isHue(record.hue) ||
    !isRole(record.role) ||
    typeof record.createdAt !== 'string' ||
    !Number.isFinite(Date.parse(record.createdAt))
  ) {
    return undefined
  }
  return {
    teammateId: record.teammateId,
    name: (record.name as string).trim(),
    hue: record.hue,
    role: record.role,
    ...(roleTitleFor(record.role, record.roleTitle) === undefined ? {} : { roleTitle: roleTitleFor(record.role, record.roleTitle) }),
    ...(parsedStarters(record.starters) === undefined ? {} : { starters: parsedStarters(record.starters) }),
    // Only a literal true: a malformed record cannot move a teammate onto a branch.
    ...(record.worktree === true ? { worktree: true } : {}),
    // An absolute path or nothing. A relative one would resolve against
    // whatever the app's own process happens to be standing in, which is not
    // a folder anybody chose.
    ...(isTeammateFolder(record.folder) ? { folder: record.folder } : {}),
    ...(parsedConnectors(record.connectors).length === 0 ? {} : { connectors: parsedConnectors(record.connectors) }),
    ...(record.ownBrowser === true ? { ownBrowser: true } : {}),
    // A record from before faces were persisted gets the face its id seeds --
    // the same face every reader would derive, so nothing changes on upgrade.
    avatar: isAvatarSpec(record.avatar) ? record.avatar : seedAvatar(record.teammateId),
    createdAt: record.createdAt,
    ...(isTeammateRoute(record.route) ? { route: record.route } : {}),
    // A mission id or nothing: a hub pointing at a string that is not one
    // would be a face that opens nothing.
    ...(safeId(record.hubMissionId) ? { hubMissionId: record.hubMissionId } : {}),
    // A limit that does not read is NO limit, the same as a record from
    // before limits existed -- never zero, which would refuse every run.
    ...(isMonthlyLimit(record.monthlyLimitUsd) ? { monthlyLimitUsd: record.monthlyLimitUsd } : {}),
    // Instructions that do not read are dropped, not the teammate (0.706).
    ...(parsedInstructions(record.instructions) === undefined ? {} : { instructions: parsedInstructions(record.instructions) }),
    // A voice that is not one of ChatGPT's nine is dropped, not the teammate (0.746).
    ...(isChatGptVoice(record.voice) ? { voice: record.voice } : {})
  }
}

/**
 * A RECORD that does not parse is dropped; a FILE that does not parse -- or
 * one from a schema this build does not know -- is unreadable, never empty.
 */
function parsedFile(text: string): StoredFile {
  let value: unknown
  try {
    value = JSON.parse(text) as unknown
  } catch {
    throw new Error(TEAMMATES_UNREADABLE)
  }
  if (typeof value !== 'object' || value === null) throw new Error(TEAMMATES_UNREADABLE)
  const record = value as Record<string, unknown>
  if (record.schemaVersion !== SCHEMA_VERSION) throw new Error(TEAMMATES_UNREADABLE)

  const teammates: PublicTeammate[] = []
  let dropped = 0
  if (Array.isArray(record.teammates)) {
    for (const entry of record.teammates.slice(0, MAX_TEAMMATES)) {
      const teammate = parsedTeammate(entry)
      // A malformed record is dropped, not repaired. The rest of the roster
      // stays usable, which is the behaviour a user can actually recover from.
      if (teammate !== undefined && !teammates.some((held) => held.teammateId === teammate.teammateId)) {
        teammates.push(teammate)
      } else {
        dropped += 1
      }
    }
  }
  // Dropping quietly is right for the app -- one bad record must not cost
  // someone their roster -- but it should not be SILENT. A drive seeded two
  // teammates with roles this store does not accept, both vanished without a
  // word, and the resulting capture read like a message-threading defect for
  // an hour (2026-09-06). Say it once; the roster still loads either way.
  if (dropped > 0) console.warn(`Roster file: dropped ${String(dropped)} teammate record(s) that did not parse.`)

  /*
   * Counted as it goes, not by `Object.keys(owners).length` per entry: that
   * rebuilt the key list every iteration, so reading the roster was
   * quadratic in conversations -- 0.9 ms at 132 owners and 1.07 s at the
   * 5,000 cap, on every read, and this file is read several times per run
   * (main-process audit, 2026-09-22). One owner is added per teammate turn.
   */
  const owners: Record<string, string> = {}
  if (typeof record.missionOwners === 'object' && record.missionOwners !== null) {
    const known = new Set(teammates.map((teammate) => teammate.teammateId))
    let kept = 0
    for (const [missionId, teammateId] of Object.entries(record.missionOwners as Record<string, unknown>)) {
      if (kept >= MAX_MISSION_OWNERS) break
      if (!safeId(missionId) || !safeId(teammateId)) continue
      if (!known.has(teammateId)) continue
      // Object.entries yields each key once, so every one kept is new.
      kept += 1
      owners[missionId] = teammateId
    }
  }

  /*
   * A name somebody typed for a conversation.
   *
   * NOT in the ledger, and that is the point. The ledger is an append-only
   * record of what was actually asked and what actually happened, and a
   * title a person changed afterwards is neither -- writing it there would
   * be editing history to make a list easier to read. It lives here, beside
   * ownership, which is the other per-conversation fact the ledger does not
   * own.
   *
   * Read as untrusted like everything else in this file: a title that is not
   * a string, or is empty once trimmed, is dropped rather than drawn.
   */
  const titles: Record<string, string> = {}
  if (typeof record.missionTitles === 'object' && record.missionTitles !== null) {
    let kept = 0
    for (const [missionId, title] of Object.entries(record.missionTitles as Record<string, unknown>)) {
      if (kept >= MAX_MISSION_TITLES) break
      if (!safeId(missionId) || typeof title !== 'string') continue
      const trimmed = title.trim().slice(0, MAX_MISSION_TITLE_LENGTH)
      if (trimmed.length === 0) continue
      kept += 1
      titles[missionId] = trimmed
    }
  }

  // Settings default rather than fail: a corrupt flag must not take the
  // roster with it, and `off` is the safe reading of an unreadable switch.
  const rawSettings = record.settings
  const settings: WorkspaceSettings = {
    swarm: typeof rawSettings === 'object' && rawSettings !== null
      ? (rawSettings as Record<string, unknown>).swarm === true
      : false,
    // Only a literal false turns replies off; absent or malformed keeps the default.
    relay: typeof rawSettings === 'object' && rawSettings !== null
      ? (rawSettings as Record<string, unknown>).relay !== false
      : true,
    relayHopCap: typeof rawSettings === 'object' && rawSettings !== null
      ? parsedHopCap((rawSettings as Record<string, unknown>).relayHopCap)
      : DEFAULT_RELAY_HOP_CAP,
    // Only a literal true lets one teammate stop another's run. Absent,
    // malformed, or a file from a version before this existed all read as
    // off -- the setting DISCARDS work, so off is the answer nobody regrets.
    interrupt: typeof rawSettings === 'object' && rawSettings !== null
      ? (rawSettings as Record<string, unknown>).interrupt === true
      : false,
    memoryMode: typeof rawSettings === 'object' && rawSettings !== null
      ? parsedMemoryMode((rawSettings as Record<string, unknown>).memoryMode)
      : DEFAULT_MEMORY_MODE,
    // Only a literal true switches Auto on. Absent, malformed, or a file from
    // an older version all read as off, which is the answer nobody regrets.
    autoMode: typeof rawSettings === 'object' && rawSettings !== null
      ? (rawSettings as Record<string, unknown>).autoMode === true
      : false,
    // Only a literal true, like Auto: a file from before this field reads as
    // the ordinary state, which is not asking.
    askConnectors: typeof rawSettings === 'object' && rawSettings !== null
      ? (rawSettings as Record<string, unknown>).askConnectors === true
      : false,
    /*
     * ON unless it was turned off.
     *
     * Colin, 2026-09-14: "plans are off by default, it should be on unless
     * we find issues." `=== true` read an ABSENT key as off, so flipping the
     * default would have changed nothing for anybody who already has a
     * settings file -- which is everybody who has run this. Absent means
     * never chosen, and never chosen means the default.
     */
    keepATodoList: parsedTodoList(
      typeof rawSettings === 'object' && rawSettings !== null
        ? (rawSettings as Record<string, unknown>).keepATodoList
        : undefined
    ),
    layout: typeof rawSettings === 'object' && rawSettings !== null
      ? parsedLayout((rawSettings as Record<string, unknown>).layout)
      : 'auto',
    tube: typeof rawSettings === 'object' && rawSettings !== null
      ? parsedTube((rawSettings as Record<string, unknown>).tube)
      : 'full',
    ...parsedMetal(rawSettings),
    terminalFaces: parsedTerminalFaces(rawSettings),
    plush: typeof rawSettings === 'object' && rawSettings !== null && (rawSettings as Record<string, unknown>).plush === true,
    claudeOwnSkills: typeof rawSettings === 'object' && rawSettings !== null && (rawSettings as Record<string, unknown>).claudeOwnSkills === true,
    replySize: typeof rawSettings === 'object' && rawSettings !== null
      ? parsedReplySize((rawSettings as Record<string, unknown>).replySize)
      : 'standard',
    ...(() => {
      const checkCommands = typeof rawSettings === 'object' && rawSettings !== null
        ? parsedCheckCommands((rawSettings as Record<string, unknown>).checkCommands)
        : undefined
      return checkCommands === undefined ? {} : { checkCommands }
    })(),
    // Each folder's most fixes when its check fails (0.812): only whole numbers from 1 to 5 are kept.
    ...(() => {
      const checkFixes = typeof rawSettings === 'object' && rawSettings !== null
        ? parsedCheckFixes((rawSettings as Record<string, unknown>).checkFixes)
        : undefined
      return checkFixes === undefined ? {} : { checkFixes }
    })(),
    ...(() => {
      const aboutYou = typeof rawSettings === 'object' && rawSettings !== null
        ? parsedAboutYou((rawSettings as Record<string, unknown>).aboutYou)
        : undefined
      return aboutYou === undefined ? {} : { aboutYou }
    })(),
    ...(() => {
      const aboutYouSuggestions = typeof rawSettings === 'object' && rawSettings !== null
        ? parsedAboutYouSuggestions((rawSettings as Record<string, unknown>).aboutYouSuggestions)
        : undefined
      return aboutYouSuggestions === undefined ? {} : { aboutYouSuggestions }
    })(),
    ...(typeof rawSettings === 'object' && rawSettings !== null && isTeammateRoute((rawSettings as Record<string, unknown>).newChatRoute)
      ? { newChatRoute: (rawSettings as Record<string, unknown>).newChatRoute as TeammateRoute }
      : {}),
    // Who carries the work on when a plan runs out (0.774); anything but a known agent is no choice.
    ...(() => {
      const planRunsOut = typeof rawSettings === 'object' && rawSettings !== null
        ? parsedPlanRunsOut((rawSettings as Record<string, unknown>).planRunsOut)
        : undefined
      return planRunsOut === undefined ? {} : { planRunsOut }
    })()
  }

  // Pins, read as untrusted as titles are: ids only, each once, no more than the cap (0.729).
  const pins: string[] = []
  if (Array.isArray(record.missionPins)) {
    for (const missionId of record.missionPins as unknown[]) {
      if (pins.length >= MAX_MISSION_PINS) break
      if (typeof missionId === 'string' && safeId(missionId) && !pins.includes(missionId)) pins.push(missionId)
    }
  }

  // Settled and snoozed, read as untrusted as the rest: ids, real times, no more than the cap (0.730).
  const settled: Record<string, MissionSettled> = {}
  if (typeof record.missionSettled === 'object' && record.missionSettled !== null) {
    let kept = 0
    for (const [missionId, entry] of Object.entries(record.missionSettled as Record<string, unknown>)) {
      if (kept >= MAX_MISSION_SETTLED) break
      if (!safeId(missionId) || typeof entry !== 'object' || entry === null) continue
      const { at, until } = entry as { at?: unknown; until?: unknown }
      if (typeof at !== 'string' || Number.isNaN(Date.parse(at))) continue
      kept += 1
      settled[missionId] = typeof until === 'string' && !Number.isNaN(Date.parse(until)) ? { at, until } : { at }
    }
  }

  return { schemaVersion: SCHEMA_VERSION, teammates, missionOwners: owners, missionTitles: titles, missionPins: pins, missionSettled: settled, settings }
}

export function createTeammateStore(options: { readonly rootDirectory: string; readonly organizationPolicy?: OrganizationPolicyState }): TeammateStore {
  const rootDirectory = options.rootDirectory
  if (!isAbsolute(rootDirectory)) throw new Error('Teammate store directory is invalid')
  const path = join(rootDirectory, 'teammates.json')
  const managedTeammate = (teammate: PublicTeammate): PublicTeammate => {
    const monthlyLimitUsd = policyMonthlyLimit(options.organizationPolicy, teammate.monthlyLimitUsd)
    return monthlyLimitUsd === undefined ? teammate : { ...teammate, monthlyLimitUsd }
  }
  const managedSettings = (settings: WorkspaceSettings): WorkspaceSettings => policyModeAllowed(options.organizationPolicy, 'auto') ? settings : { ...settings, autoMode: false }
  const checkLimit = (value: unknown): void => {
    const state = options.organizationPolicy
    if (state?.policy.monthlyLimitUsd !== undefined && value !== undefined && (typeof value !== 'number' || value > state.policy.monthlyLimitUsd)) {
      throw new OrganizationPolicyError(policyRefusal(state, 'monthlyLimitUsd', `A teammate's monthly limit must be ${state.policy.monthlyLimitUsd} USD or less. It cannot be removed.`))
    }
  }

  let queue: Promise<unknown> = Promise.resolve()
  const serialize = <T>(task: () => Promise<T>): Promise<T> => {
    const next = queue.then(task, task)
    queue = next.then(
      () => undefined,
      () => undefined
    )
    return next
  }

  // A file that is simply not there is genuinely empty. Anything else that
  // stops it being read is UNREADABLE, and no write goes over it.
  const read = async (): Promise<StoredFile> => {
    let text: string
    try {
      text = await readFile(path, 'utf8')
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
        return { schemaVersion: SCHEMA_VERSION, teammates: [], missionOwners: {}, missionTitles: {}, missionPins: [], missionSettled: {}, settings: DEFAULT_SETTINGS }
      }
      throw new Error(TEAMMATES_UNREADABLE)
    }
    if (Buffer.byteLength(text, 'utf8') > MAX_FILE_BYTES) throw new Error(TEAMMATES_UNREADABLE)
    return parsedFile(text)
  }

  const write = async (file: StoredFile): Promise<void> => {
    await mkdir(rootDirectory, { recursive: true, mode: 0o700 })
    // Write-and-rename, so a crash mid-write leaves the previous roster intact
    // rather than a truncated file that parses to an empty one.
    const temporary = `${path}.${randomUUID()}.tmp`
    const handle = await open(temporary, fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_EXCL, 0o600)
    try {
      await handle.writeFile(`${JSON.stringify(file, null, 2)}\n`, 'utf8')
      await handle.sync()
    } finally {
      await handle.close()
    }
    try {
      await replaceFile(temporary, path)
    } catch (error) {
      await unlink(temporary).catch(() => undefined)
      throw error
    }
  }

  return {
    list(): Promise<readonly PublicTeammate[]> {
      return serialize(async () => (await read()).teammates.map(managedTeammate))
    },

    create(input): Promise<PublicTeammate> {
      return serialize(async () => {
        if (!validName(input.name)) throw new Error('Teammate name is invalid')
        if (!isHue(input.hue)) throw new Error('Teammate hue is invalid')
        if (!isRole(input.role)) throw new Error('Teammate role is invalid')
        const file = await read()
        if (file.teammates.length >= MAX_TEAMMATES) throw new Error('Too many teammates')
        const clash = nameTaken(file.teammates, input.name)
        if (clash !== undefined) throw new TeammateNameTakenError(clash)
        if (input.avatar !== undefined && !isAvatarSpec(input.avatar)) throw new Error('Teammate avatar is invalid')
        checkLimit(input.monthlyLimitUsd)
        if (input.monthlyLimitUsd !== undefined && !isMonthlyLimit(input.monthlyLimitUsd)) throw new Error('Teammate limit is invalid')
        if (input.starters !== undefined && parsedStarters(input.starters) === undefined) throw new Error('Teammate starters are invalid')
        const teammateId = `tm_${randomUUID().replace(/-/g, '').slice(0, 24)}`
        const teammate: PublicTeammate = {
          teammateId,
          name: input.name.trim(),
          hue: input.hue,
          role: input.role,
          ...(roleTitleFor(input.role, input.roleTitle) === undefined ? {} : { roleTitle: roleTitleFor(input.role, input.roleTitle) }),
          ...(parsedStarters(input.starters) === undefined ? {} : { starters: parsedStarters(input.starters) }),
          ...(input.worktree === true ? { worktree: true } : {}),
          avatar: input.avatar === undefined ? seedAvatar(teammateId) : cleanAvatar(input.avatar),
          createdAt: new Date().toISOString(),
          ...(isMonthlyLimit(input.monthlyLimitUsd) ? { monthlyLimitUsd: input.monthlyLimitUsd } : {}),
          ...(parsedInstructions(input.instructions) === undefined ? {} : { instructions: parsedInstructions(input.instructions) }),
          ...(isChatGptVoice(input.voice) ? { voice: input.voice } : {})
        }
        await write({ ...file, teammates: [...file.teammates, teammate] })
        return managedTeammate(teammate)
      })
    },

    update(input): Promise<PublicTeammate> {
      return serialize(async () => {
        if (!safeId(input.teammateId)) throw new Error('Teammate id is invalid')
        if (!validName(input.name)) throw new Error('Teammate name is invalid')
        if (!isHue(input.hue)) throw new Error('Teammate hue is invalid')
        if (!isRole(input.role)) throw new Error('Teammate role is invalid')
        if (!isAvatarSpec(input.avatar)) throw new Error('Teammate avatar is invalid')
        checkLimit(input.monthlyLimitUsd)
        if (input.monthlyLimitUsd !== undefined && input.monthlyLimitUsd !== null && !isMonthlyLimit(input.monthlyLimitUsd)) {
          throw new Error('Teammate limit is invalid')
        }
        if (input.instructions !== undefined && input.instructions !== null && (typeof input.instructions !== 'string' || input.instructions.trim().length > MAX_TEAMMATE_INSTRUCTIONS)) {
          throw new Error('Teammate instructions are invalid')
        }
        if (input.voice !== undefined && input.voice !== null && !isChatGptVoice(input.voice)) throw new Error('Teammate voice is invalid')
        const file = await read()
        const existing = file.teammates.find((teammate) => teammate.teammateId === input.teammateId)
        if (existing === undefined) throw new Error('Unknown teammate')
        const clash = nameTaken(file.teammates, input.name, existing.teammateId)
        if (clash !== undefined) throw new TeammateNameTakenError(clash)
        // Identity is the id and the creation time; everything else is theirs
        // to change. Mission ownership is keyed by id, so it follows for free.
        const updated: PublicTeammate = {
          teammateId: existing.teammateId,
          name: input.name.trim(),
          hue: input.hue,
          role: input.role,
          ...(roleTitleFor(input.role, input.roleTitle) === undefined ? {} : { roleTitle: roleTitleFor(input.role, input.roleTitle) }),
          // Carried: no edit in the dialog touches them, and a rename must not
          // turn Sable's money questions back into her role's.
          ...(existing.starters === undefined ? {} : { starters: existing.starters }),
          ...(input.worktree === true ? { worktree: true } : {}),
          // Carried, not taken from the request: the renderer never names a
          // path, so an edit of the name or the face cannot move a teammate
          // out of the folder it works in.
          ...(existing.folder === undefined ? {} : { folder: existing.folder }),
          // Carried like the folder: an edit of the name must not widen a
          // teammate back to every connector.
          ...(existing.connectors === undefined ? {} : { connectors: existing.connectors }),
          // Carried too: an edit of the name must not sign it out of its own browser.
          ...(existing.ownBrowser === true ? { ownBrowser: true } : {}),
          avatar: cleanAvatar(input.avatar),
          createdAt: existing.createdAt,
          ...(existing.route === undefined ? {} : { route: existing.route }),
          // Carried like the route: renaming a teammate must not lose the
          // conversation their replies live in.
          ...(existing.hubMissionId === undefined ? {} : { hubMissionId: existing.hubMissionId }),
          // Set, removed (null), or -- omitted -- carried: an edit that never
          // mentions money must not lift a limit the person set.
          ...(isMonthlyLimit(input.monthlyLimitUsd)
            ? { monthlyLimitUsd: input.monthlyLimitUsd }
            : input.monthlyLimitUsd === undefined && existing.monthlyLimitUsd !== undefined
              ? { monthlyLimitUsd: existing.monthlyLimitUsd }
              : {}),
          // Set, removed (null or empty), or -- omitted -- carried, like the limit (0.706).
          ...(input.instructions === undefined
            ? existing.instructions === undefined ? {} : { instructions: existing.instructions }
            : parsedInstructions(input.instructions) === undefined ? {} : { instructions: parsedInstructions(input.instructions) }),
          // Set, removed (null), or -- omitted -- carried (0.746).
          ...(input.voice === undefined
            ? existing.voice === undefined ? {} : { voice: existing.voice }
            : isChatGptVoice(input.voice) ? { voice: input.voice } : {})
        }
        await write({
          ...file,
          teammates: file.teammates.map((teammate) => (teammate.teammateId === updated.teammateId ? updated : teammate))
        })
        return managedTeammate(updated)
      })
    },

    rememberRoute(teammateId, route): Promise<void> {
      return serialize(async () => {
        if (!safeId(teammateId) || !isTeammateRoute(route)) return
        const managed = policyTurnRefusal(options.organizationPolicy, route)
        if (managed !== undefined) throw new OrganizationPolicyError(managed)
        const file = await read()
        const existing = file.teammates.find((teammate) => teammate.teammateId === teammateId)
        if (existing === undefined) return
        // Named field by field, and the effort is one of them. It was not:
        // this line rebuilt the route from three fields and dropped the
        // fourth, so the level a person chose never reached the file, and
        // selecting the teammate restored their runtime, model and mode and
        // reset the effort every time. Colin, 2026-09-16: "my effort levels
        // are resetting."
        const kept: TeammateRoute = {
          runtime: route.runtime,
          model: route.model,
          mode: route.mode,
          ...(route.account === undefined ? {} : { account: route.account }),
          ...(route.effort === undefined ? {} : { effort: route.effort })
        }
        await write({
          ...file,
          teammates: file.teammates.map((teammate) =>
            teammate.teammateId === teammateId ? { ...teammate, route: kept } : teammate
          )
        })
      })
    },

    rememberHub(teammateId, missionId): Promise<void> {
      return serialize(async () => {
        if (!safeId(teammateId) || !safeId(missionId)) return
        const file = await read()
        if (!file.teammates.some((teammate) => teammate.teammateId === teammateId)) return
        await write({
          ...file,
          teammates: file.teammates.map((teammate) =>
            teammate.teammateId === teammateId ? { ...teammate, hubMissionId: missionId } : teammate
          )
        })
      })
    },

    setFolder(teammateId, folder): Promise<PublicTeammate> {
      return serialize(async () => {
        if (!safeId(teammateId)) throw new Error('Teammate id is invalid')
        if (folder !== undefined && !isTeammateFolder(folder)) throw new Error('That is not a folder Locust can use')
        const file = await read()
        const existing = file.teammates.find((teammate) => teammate.teammateId === teammateId)
        if (existing === undefined) throw new Error('Unknown teammate')
        // Spread-then-delete rather than a conditional spread: clearing has
        // to REMOVE the key, and `{ ...existing, folder: undefined }` would
        // leave `folder` present and undefined, which the file then carries.
        const updated: PublicTeammate =
          folder === undefined
            ? (({ folder: _dropped, ...rest }) => rest)(existing)
            : { ...existing, folder }
        await write({
          ...file,
          teammates: file.teammates.map((teammate) => (teammate.teammateId === teammateId ? updated : teammate))
        })
        return updated
      })
    },

    setConnectors(teammateId, names): Promise<PublicTeammate> {
      return serialize(async () => {
        if (!safeId(teammateId)) throw new Error('Teammate id is invalid')
        const kept = parsedConnectors(names)
        const file = await read()
        const existing = file.teammates.find((teammate) => teammate.teammateId === teammateId)
        if (existing === undefined) throw new Error('Unknown teammate')
        // Empty REMOVES the key: absent means everything, and a present empty
        // list would read as "nothing", which is not a choice anyone offered.
        const updated: PublicTeammate =
          kept.length === 0
            ? (({ connectors: _dropped, ...rest }) => rest)(existing)
            : { ...existing, connectors: kept }
        await write({
          ...file,
          teammates: file.teammates.map((teammate) => (teammate.teammateId === teammateId ? updated : teammate))
        })
        return updated
      })
    },

    setOwnBrowser(teammateId, on): Promise<PublicTeammate> {
      return serialize(async () => {
        if (!safeId(teammateId)) throw new Error('Teammate id is invalid')
        const file = await read()
        const existing = file.teammates.find((teammate) => teammate.teammateId === teammateId)
        if (existing === undefined) throw new Error('Unknown teammate')
        const updated: PublicTeammate = on === true ? { ...existing, ownBrowser: true } : (({ ownBrowser: _dropped, ...rest }) => rest)(existing)
        await write({
          ...file,
          teammates: file.teammates.map((teammate) => (teammate.teammateId === teammateId ? updated : teammate))
        })
        return updated
      })
    },

    remove(teammateId): Promise<void> {
      return serialize(async () => {
        if (!safeId(teammateId)) throw new Error('Teammate id is invalid')
        const file = await read()
        const teammates = file.teammates.filter((teammate) => teammate.teammateId !== teammateId)
        // Mission ownership follows the teammate out; a mission whose owner is
        // gone shows as unassigned rather than pointing at nothing.
        const missionOwners = Object.fromEntries(
          Object.entries(file.missionOwners).filter(([, owner]) => owner !== teammateId)
        )
        await write({ ...file, teammates, missionOwners })
      })
    },

    assignMission(teammateId, missionId): Promise<void> {
      return serialize(async () => {
        if (!safeId(teammateId) || !safeId(missionId)) throw new Error('Mission assignment is invalid')
        const file = await read()
        if (!file.teammates.some((teammate) => teammate.teammateId === teammateId)) {
          throw new Error('Unknown teammate')
        }
        // Bounded on write as well as on read. Without this the file grows
        // until it crosses the size cliff, and crossing it empties the roster.
        if (
          file.missionOwners[missionId] === undefined
          && Object.keys(file.missionOwners).length >= MAX_MISSION_OWNERS
        ) {
          throw new Error('Too many mission assignments')
        }
        await write({
          ...file,
          missionOwners: { ...file.missionOwners, [missionId]: teammateId }
        })
      })
    },

    unassignMission(missionId): Promise<void> {
      return serialize(async () => {
        if (!safeId(missionId)) throw new Error('Mission id is invalid')
        const file = await read()
        // A conversation gone for good frees its pin and its put-away mark too (0.753, bug hunt 2026-10-12, PN1): they
        // stayed in the file, where nothing shows them, and still counted toward the caps of 50 pins and 1,000 marks.
        const owned = missionId in file.missionOwners
        const pinned = file.missionPins.includes(missionId)
        const settled = missionId in file.missionSettled
        if (!owned && !pinned && !settled) return
        const { [missionId]: _owner, ...missionOwners } = file.missionOwners
        const { [missionId]: _settled, ...missionSettled } = file.missionSettled
        await write({ ...file, missionOwners, missionPins: file.missionPins.filter((id) => id !== missionId), missionSettled })
      })
    },

    missionOwners(): Promise<Readonly<Record<string, string>>> {
      return serialize(async () => (await read()).missionOwners)
    },

    missionTitles(): Promise<Readonly<Record<string, string>>> {
      return serialize(async () => (await read()).missionTitles)
    },

    renameMission(missionId, title): Promise<void> {
      return serialize(async () => {
        if (!safeId(missionId)) throw new Error('Mission id is invalid')
        const file = await read()
        const trimmed = title.trim().slice(0, MAX_MISSION_TITLE_LENGTH)
        /*
         * Clearing is renaming to nothing, not a separate verb. What the
         * conversation falls back to is the title it always had -- the first
         * line of what was typed -- which is still in the ledger and was
         * never overwritten.
         */
        if (trimmed.length === 0) {
          if (!(missionId in file.missionTitles)) return
          const { [missionId]: _gone, ...missionTitles } = file.missionTitles
          await write({ ...file, missionTitles })
          return
        }
        if (
          file.missionTitles[missionId] === undefined
          && Object.keys(file.missionTitles).length >= MAX_MISSION_TITLES
        ) {
          throw new Error('Too many renamed conversations')
        }
        await write({ ...file, missionTitles: { ...file.missionTitles, [missionId]: trimmed } })
      })
    },

    missionPins(): Promise<readonly string[]> {
      return serialize(async () => (await read()).missionPins)
    },

    pinMission(missionId, pinned): Promise<void> {
      return serialize(async () => {
        if (!safeId(missionId)) throw new Error('Mission id is invalid')
        const file = await read()
        const others = file.missionPins.filter((id) => id !== missionId)
        if (!pinned) {
          if (others.length === file.missionPins.length) return
          await write({ ...file, missionPins: others })
          return
        }
        if (others.length >= MAX_MISSION_PINS) throw new Error(`At most ${String(MAX_MISSION_PINS)} conversations can be pinned`)
        await write({ ...file, missionPins: [missionId, ...others] })
      })
    },

    missionSettled(): Promise<Readonly<Record<string, MissionSettled>>> {
      return serialize(async () => (await read()).missionSettled)
    },

    settleMission(missionId, settled): Promise<void> {
      return serialize(async () => {
        if (!safeId(missionId)) throw new Error('Mission id is invalid')
        const file = await read()
        const { [missionId]: _before, ...others } = file.missionSettled
        if (settled === undefined) {
          if (_before === undefined) return
          await write({ ...file, missionSettled: others })
          return
        }
        if (settled.until !== undefined && Number.isNaN(Date.parse(settled.until))) throw new Error('Snooze time is invalid')
        if (_before === undefined && Object.keys(others).length >= MAX_MISSION_SETTLED) throw new Error('Too many settled conversations')
        const entry: MissionSettled = settled.until === undefined ? { at: new Date().toISOString() } : { at: new Date().toISOString(), until: settled.until }
        await write({ ...file, missionSettled: { ...others, [missionId]: entry } })
      })
    },

    readSettings(): Promise<WorkspaceSettings> {
      return serialize(async () => managedSettings((await read()).settings))
    },

    writeSettings(requested: unknown): Promise<WorkspaceSettings> {
      return serialize(async () => {
        // A key in About you is refused, whoever sends it (0.433, shared/secrets.ts).
        if (typeof requested === 'object' && requested !== null) {
          const note = (requested as Record<string, unknown>).aboutYou
          const secret = typeof note === 'string' ? secretIn(note) : undefined
          if (secret !== undefined) throw new Error(aboutYouSecretRefusal(secret))
        }
        /*
         * A write is a CHANGE, laid over what is stored. L19 (the code
         * review): it replaced the whole object, so every caller had to send
         * every field -- and the ones that did not reset the rest: choosing
         * Auto reset the layout and the boot screen, and every other switch
         * reset the send button's metal. A field the request leaves out is
         * kept. Only what it sends is read, by the same rules as before, so a
         * malformed message still cannot turn anything on.
         */
        const stored = await read()
        const incoming = typeof requested === 'object' && requested !== null ? (requested as Record<string, unknown>) : {}
        const settings: Record<string, unknown> = {
          ...((stored.settings ?? {}) as unknown as Record<string, unknown>),
          ...Object.fromEntries(Object.entries(incoming).filter(([, value]) => value !== undefined)),
          ...(!policyModeAllowed(options.organizationPolicy, 'auto') ? { autoMode: false } : {})
        }
        // Only a literal true turns it on. Anything else -- absent, a string,
        // a truthy object -- is off, so a malformed message cannot enable a
        // workspace-wide setting.
        const next: WorkspaceSettings = {
          swarm: typeof settings === 'object' && settings !== null
            ? (settings as Record<string, unknown>).swarm === true
            : false,
          relay: typeof settings === 'object' && settings !== null
            ? (settings as Record<string, unknown>).relay !== false
            : true,
          relayHopCap: typeof settings === 'object' && settings !== null
            ? parsedHopCap((settings as Record<string, unknown>).relayHopCap)
            : DEFAULT_RELAY_HOP_CAP,
          interrupt: typeof settings === 'object' && settings !== null
            ? (settings as Record<string, unknown>).interrupt === true
            : false,
          memoryMode: typeof settings === 'object' && settings !== null
            ? parsedMemoryMode((settings as Record<string, unknown>).memoryMode)
            : DEFAULT_MEMORY_MODE,
          // The widest setting in the file, and the one a malformed message
          // must never be able to turn on.
          autoMode: typeof settings === 'object' && settings !== null
            ? (settings as Record<string, unknown>).autoMode === true
            : false,
          askConnectors: typeof settings === 'object' && settings !== null
            ? (settings as Record<string, unknown>).askConnectors === true
            : false,
          keepATodoList: parsedTodoList(
            typeof settings === 'object' && settings !== null
              ? (settings as Record<string, unknown>).keepATodoList
              : undefined
          ),
          tube: typeof settings === 'object' && settings !== null
            ? parsedTube((settings as Record<string, unknown>).tube)
            : 'full',
          layout: typeof settings === 'object' && settings !== null
            ? parsedLayout((settings as Record<string, unknown>).layout)
            : 'auto',
          ...parsedMetal(settings),
          terminalFaces: parsedTerminalFaces(settings),
          // Only a literal true: plush is a choice, never a default.
          plush: (settings as Record<string, unknown>).plush === true,
          // Only a literal true: the person's own skills are lent, never assumed.
          claudeOwnSkills: (settings as Record<string, unknown>).claudeOwnSkills === true,
          replySize: typeof settings === 'object' && settings !== null
            ? parsedReplySize((settings as Record<string, unknown>).replySize)
            : 'standard',
          ...(() => {
            const checkCommands = parsedCheckCommands((settings as Record<string, unknown>).checkCommands)
            return checkCommands === undefined ? {} : { checkCommands }
          })(),
          ...(() => {
            const checkFixes = parsedCheckFixes((settings as Record<string, unknown>).checkFixes)
            return checkFixes === undefined ? {} : { checkFixes }
          })(),
          // Kept across every write that does not name it; '' clears it (0.423).
          ...(() => {
            const aboutYou = parsedAboutYou((settings as Record<string, unknown>).aboutYou)
            return aboutYou === undefined ? {} : { aboutYou }
          })(),
          // Waiting suggestions (0.424); [] clears them.
          ...(() => {
            const aboutYouSuggestions = parsedAboutYouSuggestions((settings as Record<string, unknown>).aboutYouSuggestions)
            return aboutYouSuggestions === undefined ? {} : { aboutYouSuggestions }
          })(),
          // The model a new chat with nobody starts on (0.552); anything but a route is dropped.
          ...(isTeammateRoute((settings as Record<string, unknown>).newChatRoute)
            ? { newChatRoute: (settings as Record<string, unknown>).newChatRoute as TeammateRoute }
            : {}),
          // Kept across every write that does not name it; null clears it (0.774).
          ...(() => {
            const planRunsOut = parsedPlanRunsOut((settings as Record<string, unknown>).planRunsOut)
            return planRunsOut === undefined ? {} : { planRunsOut }
          })()
        }
        await write({ ...stored, settings: next })
        return managedSettings(next)
      })
    }
  }
}
