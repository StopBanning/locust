import { accountFromRoute, accountUsageKey } from '../shared/agent-accounts.js'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname } from 'node:path'

import { workspaceIdFor } from './workspace.js'
import { EVENT_WINDOW, windowEvents } from '../shared/event-window.js'
import { joinMessageFragments } from '../shared/messageFragments.js'
import { isOwnModel, monthOf, moneyOfRun, sumSpend, unpricedRunAt } from '../shared/spend.js'
import type { RunMoney, Spend } from '../shared/spend.js'
import type { UsageTurn } from '../shared/usage.js'
import type { MissionLedger, RecoveredMission, Workroom, WorkroomMessage } from '@teammate/mission-store'
import type { MissionReadResponse,
  MissionDeleteResponse,
  MissionHistoryResponse,
  PublicPeerMessage,
  PublicRecoveredMission
} from '../shared/ipc.js'
import { replaceFile } from './replace-file.js'
import { filesNamedIn, namedFileKey } from './named-files.js'

const MAX_HISTORY_MISSIONS = 20
/**
 * How many conversations the sidebar may LIST, as opposed to carry transcript
 * for.
 *
 * THE BUG THIS EXISTS FOR. 0.216 stopped a paging boundary changing what a
 * conversation IS, by fetching absent ancestors. It did not stop a whole
 * conversation falling off the page: `listMissions` returns the 20
 * most-recently-touched missions, and a conversation whose own newest turn is
 * older than those twenty is simply not in the response. No row, no group
 * entry, nothing -- Colin, 2026-09-20, on the same conversation for the
 * second time: *"my research convo disappeared again"*.
 *
 * Twenty was never a statement about how many conversations a person may
 * have. It is a budget on TRANSCRIPT: a mission carries up to 500 events and
 * some of those are megabytes, which is what `MAX_HISTORY_BYTES` guards. A
 * row in the list needs none of that -- an id, a prompt, a phase and a clock,
 * a few hundred bytes -- so the two limits had no business being one number.
 *
 * They are now two. The newest `MAX_HISTORY_MISSIONS` come with their events;
 * everything up to here comes with `events: []`. One ledger read either way.
 */
/*
 * 2,000 since QA-2026-09-29 round 2, R28: at 300 -- a turn is a mission, so a
 * few weeks of steady use -- older conversations vanished from the list, from
 * Ctrl K and from a teammate's card, and the header read like a total. The
 * cliff has moved again, so it is SAID now: past it, the history names how
 * many the ledger holds and the Missions header says "the newest N of M".
 */
const MAX_LISTED_MISSIONS = 2_000
// The same window the live thread keeps (0.627, shared/event-window.ts): 500 dropped a long run's
// messages and early steps from the screen while the record still held them.
export const MAX_HISTORY_EVENTS = EVENT_WINDOW
const MAX_HISTORY_CHECKPOINTS = 25
/**
 * Ceiling on one IPC response. The per-mission event window bounds COUNT, not
 * SIZE: a mission of large tool outputs can carry megabytes inside 500 events,
 * and twenty such missions would hand the renderer an unbounded payload to
 * structured-clone in one go. Missions are dropped from the tail -- they are
 * already ordered newest first, so what is lost is the oldest.
 */
const MAX_HISTORY_BYTES = 4 * 1024 * 1024

/**
 * Join a mission's peer links with the workroom's messages. The ledger holds
 * only ids, so the text comes from the channel; a link whose message is gone
 * is kept with `text: null` rather than dropped, because "this mission was
 * shown a message that can no longer be read" is itself worth knowing.
 */
export function publicPeerMessages(
  mission: RecoveredMission,
  workroomMessages: ReadonlyMap<string, WorkroomMessage>
): readonly PublicPeerMessage[] {
  return mission.peerLinks.map((link) => {
    const message = workroomMessages.get(link.messageId)
    if (message === undefined) {
      const unknown = { teammateId: link.peerTeammateId, name: 'a teammate' }
      return {
        messageId: link.messageId,
        direction: link.direction,
        from: link.direction === 'received' ? unknown : { teammateId: '', name: '' },
        to: link.direction === 'received' ? { teammateId: '', name: '' } : unknown,
        text: null,
        at: link.occurredAt
      }
    }
    return {
      messageId: message.messageId,
      direction: link.direction,
      // With the conversation it was sent from (0.463): a reopened conversation
      // links back to it, and the sidebar draws it underneath. The live path
      // always carried it; the record's projection dropped it.
      from: {
        teammateId: message.from.teammateId,
        name: message.from.name,
        ...(message.from.missionId === undefined ? {} : { missionId: message.from.missionId })
      },
      to: { teammateId: message.to.teammateId, name: message.to.name },
      text: message.text,
      at: message.postedAt
    }
  })
}

/**
 * Which runtimes are still, as far as the record knows, at their limit.
 *
 * Per runtime, the latest event in time across all missions decides: a
 * `route.limit_detected` of kind `quota-exhausted` puts it at its limit, a
 * later `run.completed` on the same runtime takes it off. Same rule the
 * window applies live, applied to the ledger at boot so a reload does not
 * amount to a lie about the account.
 */
export function limitedRuntimesFrom(missions: readonly RecoveredMission[]): Record<string, string> {
  const latest = new Map<string, { readonly at: string; readonly said: string | undefined }>()
  for (const mission of missions) {
    for (const event of mission.events) {
      let said: string | undefined
      let relevant = false
      if (event.type === 'route.limit_detected' && event.payload.kind === 'quota-exhausted') {
        relevant = true
        said = event.payload.message
      } else if (event.type === 'run.completed') {
        relevant = true
      }
      if (!relevant) continue
      const runtime = accountUsageKey(event.sourceAdapter, accountFromRoute(mission.metadata.resolvedRouteId))
      const current = latest.get(runtime)
      if (current === undefined || event.occurredAt > current.at) latest.set(runtime, { at: event.occurredAt, said })
    }
  }
  const limited: Record<string, string> = {}
  for (const [runtime, entry] of latest) {
    if (entry.said !== undefined) limited[runtime] = entry.said
  }
  return limited
}

/**
 * The latest still-allowed rate-limit reading per runtime, from the ledger,
 * so a reload does not forget what the route chip said. A limit that hit
 * (`route.limit_detected`) is the other map; this one is the number before it.
 */
/**
 * A usage WARNING read back as a reading (0.407): "You've used 79% of your
 * 7-day window · resets <iso>" is "7-day window 79% used · resets <iso>".
 * Before 0.407 a warning was kept only as the notice, so a ledger written
 * then holds its figure only here -- Colin's did, nine times, the morning his
 * card still said 66%.
 */
export function readingOfWarning(message: string): string | undefined {
  const found = /^You've used (\d{1,3})% of your (.+?) window(?: · resets (\S+))?$/.exec(message)
  if (found === null) return undefined
  return `${found[2]!} window ${found[1]!}% used${found[3] === undefined ? '' : ` · resets ${found[3]}`}`
}

export function usageWindowsFrom(missions: readonly RecoveredMission[]): Record<string, string> {
  const latest = new Map<string, { readonly at: string; readonly said: string }>()
  for (const mission of missions) {
    for (const event of mission.events) {
      const said = event.type === 'adapter.diagnostic' && /\.usage_window$/.test(event.payload.code)
        ? event.payload.message
        : event.type === 'route.limit_detected' && event.payload.kind === 'temporary-rate-limit'
          ? readingOfWarning(event.payload.message)
          : undefined
      if (said === undefined) continue
      const runtime = accountUsageKey(event.sourceAdapter, accountFromRoute(mission.metadata.resolvedRouteId))
      const current = latest.get(runtime)
      if (current === undefined || event.occurredAt > current.at) latest.set(runtime, { at: event.occurredAt, said })
    }
  }
  const windows: Record<string, string> = {}
  // 0.406: with WHEN it was seen. A reading is only what the last run in
  // Locust reported; shown bare, last night's 20% read as now (Colin,
  // 2026-09-27: "my claude code usage hasnt seem to have updated").
  for (const [runtime, entry] of latest) windows[runtime] = `${entry.said} · from a run at ${entry.at}`
  return windows
}

/** When a reading was seen: the "from a run at <iso>" it carries. */
function readingAt(said: string): string {
  return / · from a run at (\S+)$/.exec(said)?.[1] ?? ''
}

/**
 * EACH AGENT'S LAST USAGE READING, KEPT (0.571).
 *
 * Colin, 2026-10-03: "claude code on home stopped showing usage". The
 * reading was read only off the newest twenty conversations, and an evening
 * of Codex and Antigravity runs had pushed his last Claude run to the
 * fortieth. Its account had not stopped having a window; Locust had stopped
 * looking. The newest reading of each agent is now kept in a small file, the
 * newer of the kept and the fresh one wins, and a file that does not exist yet
 * is filled once from the conversations past the first twenty.
 */
export async function rememberedUsageWindows(
  fresh: Record<string, string>,
  file: string,
  older: () => Promise<readonly RecoveredMission[]>
): Promise<Record<string, string>> {
  let kept: Record<string, string> | undefined
  try {
    const parsed = JSON.parse(await readFile(file, 'utf8')) as unknown
    if (parsed !== null && typeof parsed === 'object' && !Array.isArray(parsed)) {
      kept = Object.fromEntries(Object.entries(parsed as Record<string, unknown>).filter((entry): entry is [string, string] => typeof entry[1] === 'string'))
    }
  } catch {
    kept = undefined
  }
  const merged: Record<string, string> = { ...(kept ?? usageWindowsFrom(await older().catch(() => []))) }
  for (const [runtime, said] of Object.entries(fresh)) {
    const before = merged[runtime]
    if (before === undefined || readingAt(said) >= readingAt(before)) merged[runtime] = said
  }
  if (kept === undefined || JSON.stringify(kept) !== JSON.stringify(merged)) {
    try {
      await mkdir(dirname(file), { recursive: true })
      await writeFile(`${file}.tmp`, JSON.stringify(merged), 'utf8')
      await replaceFile(`${file}.tmp`, file)
    } catch {
      // Not kept this time: the reading shown is still right, and the next load tries again.
    }
  }
  return merged
}

/** How far back a first fill looks for a reading. */
const USAGE_BACKFILL_MISSIONS = 80

/** A run's money as a row carries it: the amounts, not the moment. */
function amountsOf(money: RunMoney | undefined): Spend | undefined {
  if (money === undefined) return undefined
  return {
    ...(money.usd === undefined ? {} : { usd: money.usd }),
    ...(money.premiumRequests === undefined ? {} : { premiumRequests: money.premiumRequests })
  }
}

export function publicRecoveredMission(
  mission: RecoveredMission,
  workroomMessages: ReadonlyMap<string, WorkroomMessage> = new Map()
): PublicRecoveredMission {
  // Joined BEFORE the window, never after: windowing fragments is what lost
  // the front of a reply. See `joinMessageFragments`.
  const whole = joinMessageFragments(mission.events)
  const events = windowEvents(whole, MAX_HISTORY_EVENTS)
  const hostFailureMessage = mission.hostFailures.at(-1)?.message
  const money = amountsOf(moneyOfRun(mission.events))
  return {
    missionId: mission.metadata.missionId,
    runId: mission.metadata.runId,
    // Which folder it ran in, so the shell can open the conversation that
    // belongs to the folder it was launched in.
    workspaceId: mission.metadata.workspaceId,
    prompt: mission.metadata.prompt,
    runtime: mission.metadata.runtime,
    model: mission.metadata.model,
    requestedRouteId: mission.metadata.requestedRouteId,
    resolvedRouteId: mission.metadata.resolvedRouteId,
    cliVersion: mission.metadata.cliVersion,
    sandbox: mission.metadata.sandbox,
    // What was ASKED for, beside what it was allowed. Without it a reopened
    // plan is indistinguishable from an ordinary read-only run -- the ledger
    // gained the field and the projection did not pass it on, which is the
    // half a unit test cannot see and a drive can (2026-09-06).
    ...(mission.metadata.mode === undefined ? {} : { mode: mission.metadata.mode }),
    createdAt: mission.metadata.createdAt,
    lastUpdatedAt: mission.lastUpdatedAt,
    phase: mission.phase,
    events,
    eventCount: mission.events.length,
    // Read from the WHOLE record, here, so a row sent without its events
    // still says what it cost: the window's totals were adding up the
    // newest twenty and calling it everything (2026-09-26).
    ...(money === undefined ? {} : { money }),
    // What the person is told about. Joining fragments loses nothing, so it
    // is not truncation; only the window below it drops anything.
    eventsTruncated: events.length !== whole.length,
    ...(hostFailureMessage === undefined ? {} : { hostFailureMessage }),
    integrityIssueCount: mission.issues.length,
    // Bounded projection: counts and causes, not the digest or the summary.
    checkpoints: mission.checkpoints.slice(-MAX_HISTORY_CHECKPOINTS).map((checkpoint) => ({
      epoch: checkpoint.epoch,
      reason: checkpoint.reason,
      resumeSafety: checkpoint.resumeSafety,
      safetyReason: checkpoint.safetyReason,
      createdAt: checkpoint.createdAt,
      unsettledActions: checkpoint.unsettledActions.map((action) => ({
        itemId: action.itemId,
        name: action.name
      }))
    })),
    peerMessages: publicPeerMessages(mission, workroomMessages),
    // Copied field by field, like everything else that leaves for the window.
    ...(mission.editChecks.at(-1) === undefined
      ? {}
      : {
          editCheck: {
            command: mission.editChecks.at(-1)!.command,
            outcome: mission.editChecks.at(-1)!.outcome,
            newLines: mission.editChecks.at(-1)!.newLines,
            unchanged: mission.editChecks.at(-1)!.unchanged,
            first: mission.editChecks.at(-1)!.first
          }
        }),
    ...(mission.metadata.continuesFrom === undefined
      ? {}
      : {
          continuesFrom: {
            missionId: mission.metadata.continuesFrom.missionId,
            checkpointEpoch: mission.metadata.continuesFrom.checkpointEpoch,
            reason: mission.metadata.continuesFrom.reason,
            ...(mission.metadata.continuesFrom.edited === true ? { edited: true as const } : {}),
            ...(mission.metadata.continuesFrom.leftOut === undefined || mission.metadata.continuesFrom.leftOut.length === 0 ? {} : { leftOut: mission.metadata.continuesFrom.leftOut }),
            ...(mission.metadata.continuesFrom.leftOutByYou === undefined || mission.metadata.continuesFrom.leftOutByYou.length === 0 ? {} : { leftOutByYou: mission.metadata.continuesFrom.leftOutByYou })
          }
        }),
    // Copied by kind rather than spread: each carries a different counter, and
    // a spread would let a future field ride out to the renderer unreviewed.
    ...(mission.metadata.startedBy === undefined
      ? {}
      : mission.metadata.startedBy.kind === 'mcp'
        // A voice call's hand-off says so (0.739); nothing but that one word rides along.
        ? { startedBy: mission.metadata.startedBy.via === 'voice' ? { kind: 'mcp' as const, via: 'voice' as const } : { kind: 'mcp' as const } }
      : mission.metadata.startedBy.kind === 'relay'
        ? { startedBy: { kind: 'relay' as const, hop: mission.metadata.startedBy.hop } }
        : mission.metadata.startedBy.kind === 'routine'
          ? {
              startedBy: {
                kind: 'routine' as const,
                routineId: mission.metadata.startedBy.routineId,
                step: mission.metadata.startedBy.step
              }
            }
          : mission.metadata.startedBy.kind === 'terminal'
            ? { startedBy: { kind: 'terminal' as const, exchange: mission.metadata.startedBy.exchange } }
            : mission.metadata.startedBy.kind === 'side'
              ? { startedBy: { kind: 'side' as const, of: mission.metadata.startedBy.of, question: mission.metadata.startedBy.question } }
              : mission.metadata.startedBy.kind === 'carry-on'
                ? { startedBy: { kind: 'carry-on' as const, from: mission.metadata.startedBy.from } }
                : mission.metadata.startedBy.kind === 'check-fix'
                  ? { startedBy: { kind: 'check-fix' as const, fix: mission.metadata.startedBy.fix, of: mission.metadata.startedBy.of, command: mission.metadata.startedBy.command } }
                  : { startedBy: { kind: 'resume' as const, epoch: mission.metadata.startedBy.epoch } })
  }
}

/*
 * `unreadableFileCount` lived here and is gone.
 *
 * It counted ledger files that raised an issue and yielded no mission, by
 * asking which issue ids were absent from `snapshot.missions`. That is right
 * only if `missions` is every mission the reader recovered -- and it is not:
 * `listMissions` slices it to a page. An older mission that recovered
 * perfectly and simply fell outside the twenty newest therefore had its issue
 * counted as a file that could not be read, and the screen said
 * "20 local - 1 file could not be read" about a ledger with nothing wrong.
 *
 * Found by Astra, 2026-09-09, in the fix I had just shipped for their previous
 * finding. Crying wolf is the same disease as false reassurance pointed the
 * other way, and it is worse here: a damage warning is only worth having if it
 * is rare enough to believe.
 *
 * The count now comes from `MissionLedgerSnapshot.unreadableCount`, computed
 * inside the reader from its parse results before any slicing -- the one place
 * where whether a file produced a mission is actually known.
 */

/**
 * Take missions until the response would exceed its byte budget. The first
 * mission is always included even if it alone is over budget: returning an
 * empty history for one large mission would look like "you have no missions",
 * which is a worse failure than a large payload.
 */
export function withinByteBudget(
  missions: readonly PublicRecoveredMission[],
  budget = MAX_HISTORY_BYTES
): readonly PublicRecoveredMission[] {
  return budgeted(missions, budget).map((entry) => entry.mission)
}

/** `withinByteBudget`, keeping each mission's JSON: the digest is taken from it. */
function budgeted(
  missions: readonly PublicRecoveredMission[],
  budget = MAX_HISTORY_BYTES
): readonly { readonly mission: PublicRecoveredMission; readonly json: string }[] {
  const kept: { readonly mission: PublicRecoveredMission; readonly json: string }[] = []
  let used = 0
  for (const mission of missions) {
    const json = JSON.stringify(mission)
    const size = Buffer.byteLength(json, 'utf8')
    if (kept.length > 0 && used + size > budget) break
    kept.push({ mission, json })
    used += size
  }
  return kept
}

/**
 * THE WINDOW ALREADY HAS MOST OF THIS.
 *
 * The history is read on every run end and every first open of a finished
 * conversation, and each read sent the newest twenty missions whole: 2.69 MB
 * a read on Colin's 138-mission ledger (Batch C, 2026-09-22), almost all of
 * it the same events as the read before. Structured-cloned across the IPC
 * boundary, parsed, and then handed to React as new objects -- so every
 * earlier turn on screen, memoised on its events since 0.262, was built
 * again for nothing.
 *
 * So a whole record carries a digest of itself as sent, the window hands the
 * digests of what it holds back with its next read, and a record whose
 * digest has not moved comes back with `events: []` and `eventsKept`. The
 * window keeps its own copy -- the same object, so nothing built from it is
 * rebuilt. The digest is of the PROJECTION, not the ledger file, because the
 * projection also carries what the workroom said, which changes on its own.
 */
export function missionDigest(json: string): string {
  return createHash('sha1').update(json, 'utf8').digest('hex').slice(0, 24)
}

/** How many held records a window may name in one read -- far past what one read returns. */
const MAX_KNOWN = 1000

/**
 * What the window says it holds, or nothing when it says it in a shape this
 * does not recognise. Anything odd is simply a full read: the only cost of
 * ignoring this is the bytes it would have saved.
 */
export function knownDigests(value: unknown): ReadonlyMap<string, string> | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const entries = Object.entries(value as Record<string, unknown>)
  if (entries.length === 0 || entries.length > MAX_KNOWN) return undefined
  const known = new Map<string, string>()
  for (const [missionId, digest] of entries) {
    if (typeof digest !== 'string' || digest.length === 0 || digest.length > 64 || missionId.length > 200) return undefined
    known.set(missionId, digest)
  }
  return known
}

/** Bound on the extra ledger reads one history call may do to find roots. */
const MAX_ANCESTOR_READS = 200

/**
 * The earlier turns of every conversation on the page, as IDENTITY ONLY.
 *
 * THE BUG THIS EXISTS FOR, and it is the worst kind this app has shipped:
 * a paging boundary was changing what a conversation IS.
 *
 * Colin, 2026-09-20, with a screenshot of an empty group: *"both my stonks
 * and research chats are gone out of the stonks group, probably autorenamed
 * and moved, def need this fixed"*. Nothing was moved and nothing was
 * renamed. His ledger had 58 missions; this call returns the 20 most recently
 * touched; and his stonks conversations began on 14 and 15 September, so
 * their EARLIEST turns fell off the page. The renderer builds a row by
 * walking `continuesFrom` back through the missions it was given, so with
 * those turns absent the walk stopped early and the row's `rootId` became a
 * later turn. Everything hangs off that id:
 *
 *   - the title is the ROOT's first sentence, so the row renamed itself to a
 *     mid-conversation prompt — "looks autorenamed";
 *   - a name he typed is stored against the root, so it stopped matching;
 *   - group membership is stored against the root, so `stonks` matched
 *     nothing and read "Nothing in here yet" — "looks moved".
 *
 * Three symptoms, one cause, and no data involved in any of them: the
 * ledgers and `groups.json` were intact the whole time. Measured against his
 * real files before a line was changed.
 *
 * WHY IDENTITY ONLY. The page is bounded by bytes because a mission carries
 * up to 500 events and some of those are megabytes; raising the count would
 * just move the cliff. But a row needs its ancestors' IDS AND PROMPTS, not
 * their transcripts — a few hundred bytes each — so these are projected with
 * `events` emptied and appended after the budget. They cost nothing and they
 * make the row's identity independent of where the page happened to cut.
 *
 * This is a floor, not the whole answer. The real fix is a compact
 * conversation list that is not the same payload as the thread, so the
 * sidebar stops being a side effect of how much transcript fits in one IPC
 * message. Until then a very old conversation can still be short of turns
 * when opened — but it is in the right group, under its own name.
 */
async function ancestorsOf(
  shown: readonly PublicRecoveredMission[],
  ledger: MissionLedger,
  workroomMessages: ReadonlyMap<string, WorkroomMessage>
): Promise<readonly PublicRecoveredMission[]> {
  const have = new Set(shown.map((mission) => mission.missionId))
  const wanted = shown.flatMap((mission) =>
    mission.continuesFrom === undefined ? [] : [mission.continuesFrom.missionId])
  const found: PublicRecoveredMission[] = []
  let reads = 0
  while (wanted.length > 0 && reads < MAX_ANCESTOR_READS) {
    const missionId = wanted.shift()
    if (missionId === undefined || have.has(missionId)) continue
    have.add(missionId)
    reads += 1
    let older
    try {
      older = await ledger.getMission(missionId)
    } catch {
      // A torn or missing ancestor ends this branch of the walk where it
      // stands. The row keeps whatever root it can reach, which is exactly
      // the old behaviour rather than a new failure.
      continue
    }
    if (older === undefined) continue
    found.push({ ...publicRecoveredMission(older, workroomMessages), events: [] })
    const next = older.metadata.continuesFrom?.missionId
    if (next !== undefined) wanted.push(next)
  }
  return found
}

/**
 * What one ledger file contributed to the last read, kept by the file's
 * identity so an unchanged file is never parsed twice.
 *
 * The history is asked for on every run end and every first open of a
 * finished conversation, and each ask parsed EVERY ledger file: 490-560 ms
 * on Colin's 138-mission ledger, measured in the app (2026-09-22), with the
 * main process slow to answer anything else meanwhile. Nearly every file is
 * the same as last time.
 *
 * Held LIGHT: a parsed ledger kept whole is 63.6 MB for that same ledger
 * (measured), so everything but the newest few keeps its record with the
 * events dropped, plus the numbers a row needs from them -- including what
 * the run cost, which the monthly totals read (`spendByTeammate`).
 */
interface CachedLedgerFile {
  readonly missionId: string
  readonly stamp: string
  readonly issues: number
  /** The record without its events; absent when the file produced no mission. */
  readonly light?: RecoveredMission
  readonly eventCount: number
  readonly eventsTruncated: boolean
  /** What the run cost in money, and when it ended; absent when it reported none. */
  readonly money?: RunMoney
  /** When a run on a model of the person's own ended with tokens and no price (0.689). */
  readonly unpricedAt?: string
  /** What the turn counts for in Usage (0.714): its receipt, and the turn it continues. */
  readonly turn?: UsageTurnRead
  /** The whole paths its record names as the teammate's (0.787, named-files.ts), as keys. */
  readonly named?: readonly string[]
  /** The whole record, kept only while it is among the newest. */
  full?: RecoveredMission
}

interface HistoryPage {
  /** Newest first: whole records for the first few, light ones after. */
  readonly missions: readonly {
    readonly mission: RecoveredMission
    readonly eventCount?: number
    readonly eventsTruncated?: boolean
    readonly money?: RunMoney
  }[]
  readonly issueCount: number
  /** Every mission the ledger could read, before the page was cut. */
  readonly totalCount?: number
  readonly unreadableCount: number
}

const ledgerCaches = new WeakMap<MissionLedger, Map<string, CachedLedgerFile>>()

async function eachLimited<T>(items: readonly T[], limit: number, work: (item: T) => Promise<void>): Promise<void> {
  let next = 0
  const runners = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (next < items.length) {
      const item = items[next]
      next += 1
      if (item !== undefined) await work(item)
    }
  })
  await Promise.all(runners)
}

/**
 * Every ledger file's cached entry, brought up to date: a file that changed
 * since the last read is parsed again, one that is gone is forgotten.
 * Undefined for a ledger that cannot list its files (a test's fake).
 */
async function refreshedLedger(ledger: MissionLedger): Promise<{
  readonly cache: Map<string, CachedLedgerFile>
  readonly entries: readonly CachedLedgerFile[]
  readonly listingIssues: number
  readonly readMission: NonNullable<MissionLedger['readMission']>
  /** Every ledger file, before the store's cap (R28). */
  readonly totalFiles: number
} | undefined> {
  if (ledger.missionFiles === undefined || ledger.readMission === undefined) return undefined
  const readMission = ledger.readMission.bind(ledger)
  const cache = ledgerCaches.get(ledger) ?? new Map<string, CachedLedgerFile>()
  ledgerCaches.set(ledger, cache)
  const listing = await ledger.missionFiles()
  const present = new Set(listing.files.map((file) => file.missionId))
  for (const missionId of [...cache.keys()]) if (!present.has(missionId)) cache.delete(missionId)

  const read = async (missionId: string, stamp: string): Promise<void> => {
    const answer = await readMission(missionId)
    const mission = answer.mission
    if (mission === undefined) {
      cache.set(missionId, { missionId, stamp, issues: answer.issues.length, eventCount: 0, eventsTruncated: false })
      return
    }
    const money = moneyOfRun(mission.events)
    const unpricedAt = unpricedRunAt(mission.events, isOwnModel(mission.metadata.runtime, mission.metadata.model))
    const turn = usageTurnOf(mission)
    const named = filesNamedIn(mission.events)
    cache.set(missionId, {
      missionId,
      stamp,
      issues: answer.issues.length,
      light: { ...mission, events: [] },
      eventCount: mission.events.length,
      eventsTruncated: joinMessageFragments(mission.events).length > MAX_HISTORY_EVENTS,
      ...(money === undefined ? {} : { money }),
      ...(unpricedAt === undefined ? {} : { unpricedAt }),
      turn,
      ...(named.length === 0 ? {} : { named }),
      full: mission
    })
  }
  const stampOf = (file: { readonly modifiedAt: number; readonly size: number }): string => `${String(file.modifiedAt)}:${String(file.size)}`
  const changed = listing.files.filter((file) => cache.get(file.missionId)?.stamp !== stampOf(file))
  await eachLimited(changed, 8, (file) => read(file.missionId, stampOf(file)))

  const entries = listing.files.flatMap((file) => {
    const entry = cache.get(file.missionId)
    return entry === undefined ? [] : [entry]
  })
  return { cache, entries, listingIssues: listing.issues.length, readMission, totalFiles: listing.totalFiles ?? listing.files.length }
}

/**
 * Whether a teammate's recorded turn names this whole path as its own (0.787,
 * named-files.ts). Through the history's cache, which reads a file again
 * when it changes: a turn still running is read as it grows, so a file it
 * handed over a moment ago is found.
 */
export async function namedByATeammate(ledger: MissionLedger, path: string): Promise<boolean> {
  const key = namedFileKey(path)
  if (key === undefined) return false
  const refreshed = await refreshedLedger(ledger)
  return refreshed?.entries.some((entry) => entry.named?.includes(key) === true) ?? false
}

/** A turn's usage as its file was read: everything but which conversation it is in, found from the whole ledger. */
type UsageTurnRead = Omit<UsageTurn, 'conversation'> & { readonly continues?: string }

function tokenCount(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : undefined
}

/**
 * What one turn counts for in Usage (0.714): when it ended, which model ran
 * it -- the one its run reported, where it reported one (Claude Code says
 * `claude-opus-4-6` for `opus`) -- and its receipt's tokens and money, read
 * by the same rule as the cost line (`moneyOfUsage`, through moneyOfRun). A
 * turn with no receipt still counts, at the last moment its record moved.
 */
export function usageTurnOf(mission: RecoveredMission): UsageTurnRead {
  const completed = mission.events.find((event) => event.type === 'run.completed')
  const payload = (completed?.payload ?? {}) as { readonly usage?: unknown; readonly resolvedModel?: unknown }
  const usage = typeof payload.usage === 'object' && payload.usage !== null ? (payload.usage as Record<string, unknown>) : {}
  const reported = typeof payload.resolvedModel === 'string' && payload.resolvedModel.length > 0 ? payload.resolvedModel : undefined
  const money = moneyOfRun(mission.events)
  const plan = usage.billing === 'subscription'
  const counted = (key: string): Partial<Record<string, number>> => {
    const value = tokenCount(usage[key])
    return value === undefined ? {} : { [key]: value }
  }
  return {
    at: completed?.occurredAt ?? mission.lastUpdatedAt,
    runtime: mission.metadata.runtime,
    model: reported ?? mission.metadata.model,
    ...counted('inputTokens'),
    ...counted('outputTokens'),
    ...counted('cacheReadTokens'),
    ...counted('cacheWriteTokens'),
    ...(money?.usd === undefined ? {} : { usd: money.usd }),
    ...(money?.premiumRequests === undefined ? {} : { premiumRequests: money.premiumRequests }),
    ...(plan ? { plan: true as const } : {}),
    ...(mission.metadata.continuesFrom === undefined ? {} : { continues: mission.metadata.continuesFrom.missionId })
  }
}

/**
 * EVERY TURN THE LEDGER HOLDS, FOR USAGE (0.714): read through the same cache
 * as the history and the monthly spend, so after the first read it is a pass
 * over a few hundred small entries. Each turn names its conversation by the
 * conversation's first turn, found by following `continuesFrom` back. Undefined
 * when the ledger cannot be listed file by file.
 */
export async function usageTurns(ledger: MissionLedger, requireReadable = false): Promise<readonly UsageTurn[] | undefined> {
  const refreshed = await refreshedLedger(ledger)
  if (refreshed === undefined) return undefined
  if (requireReadable && (refreshed.listingIssues > 0 || refreshed.entries.some((entry) => entry.issues > 0))) return undefined
  const read = new Map<string, UsageTurnRead>()
  for (const entry of refreshed.entries) if (entry.turn !== undefined) read.set(entry.missionId, entry.turn)
  const rootOf = (missionId: string): string => {
    let at = missionId
    // Bounded, so a loop a damaged record makes cannot hang the host.
    for (let hops = 0; hops < 10_000; hops += 1) {
      const before = read.get(at)?.continues
      if (before === undefined || !read.has(before)) return at
      at = before
    }
    return at
  }
  return [...read.entries()].map(([missionId, turn]) => {
    const { continues: _continues, ...counted } = turn
    return { ...counted, conversation: rootOf(missionId) }
  })
}

/**
 * WHAT EACH TEAMMATE HAS SPENT IN ONE CALENDAR MONTH, from every
 * conversation the ledger holds -- not the newest twenty the window is sent.
 *
 * A run counts in the month it ENDED, on this machine's clock, toward the
 * teammate the host recorded it for (`missionOwners`); a run that belongs to
 * nobody counts toward nobody. Read through the same cache as the history,
 * so after the first read it costs a pass over a few hundred small entries.
 * Undefined when the ledger cannot be listed file by file.
 */
export async function spendByTeammate(
  ledger: MissionLedger,
  owners: Readonly<Record<string, string>>,
  month: string,
  requireReadable = false
): Promise<ReadonlyMap<string, Spend> | undefined> {
  const refreshed = await refreshedLedger(ledger)
  if (refreshed === undefined) return undefined
  if (requireReadable && (refreshed.listingIssues > 0 || refreshed.entries.some((entry) => entry.issues > 0))) return undefined
  const runs = new Map<string, Spend[]>()
  for (const entry of refreshed.entries) {
    const owner = owners[entry.missionId]
    if (owner === undefined) continue
    if (entry.money !== undefined && monthOf(entry.money.at) === month) runs.set(owner, [...(runs.get(owner) ?? []), entry.money])
    if (entry.unpricedAt !== undefined && monthOf(entry.unpricedAt) === month) runs.set(owner, [...(runs.get(owner) ?? []), { unpricedRuns: 1 }])
  }
  const totals = new Map<string, Spend>()
  for (const [teammateId, list] of runs) {
    const total = sumSpend(list)
    if (total !== undefined) totals.set(teammateId, total)
  }
  return totals
}

async function cachedHistoryPage(ledger: MissionLedger, limit: number): Promise<HistoryPage | undefined> {
  const refreshed = await refreshedLedger(ledger)
  if (refreshed === undefined) return undefined
  const { entries, readMission } = refreshed
  const readable = entries
    .filter((entry): entry is CachedLedgerFile & { readonly light: RecoveredMission } => entry.light !== undefined)
    .sort((left, right) => Date.parse(right.light.lastUpdatedAt) - Date.parse(left.light.lastUpdatedAt))
  // What the store listed may itself be cut short: its count of files wins.
  const totalCount = Math.max(readable.length, refreshed.totalFiles)
  readable.splice(limit)
  // Whole records for the newest, and only for them.
  const newest = new Set(readable.slice(0, MAX_HISTORY_MISSIONS).map((entry) => entry.light.metadata.missionId))
  for (const entry of entries) {
    if (entry.light !== undefined && !newest.has(entry.light.metadata.missionId)) entry.full = undefined
  }
  // One that moved INTO the newest without changing (another was deleted)
  // has no whole record yet: read it again.
  await eachLimited(
    readable.filter((entry) => newest.has(entry.light.metadata.missionId) && entry.full === undefined),
    8,
    async (entry) => {
      const again = await readMission(entry.light.metadata.missionId)
      if (again.mission !== undefined) entry.full = again.mission
    }
  )
  return {
    missions: readable.map((entry) =>
      entry.full !== undefined
        ? { mission: entry.full }
        : {
            mission: entry.light,
            eventCount: entry.eventCount,
            eventsTruncated: entry.eventsTruncated,
            ...(entry.money === undefined ? {} : { money: entry.money })
          }
    ),
    issueCount: refreshed.listingIssues + entries.reduce((total, entry) => total + entry.issues, 0),
    unreadableCount: entries.filter((entry) => entry.light === undefined).length,
    totalCount
  }
}

/**
 * H3: ONE MISSION, WHOLE. History sends the newest missions with their
 * events and every other one as a row, promising that opening it fetches
 * the rest (0.221.0) -- and nothing did, so an older conversation opened
 * with the person's own words and no replies. This is that fetch: the same
 * projection, the same workroom join, the same digest a history read gives.
 */
export async function readOneMission(ledger: MissionLedger, workroom: Workroom | undefined, missionId: unknown): Promise<MissionReadResponse> {
  const unavailable = { ok: false, error: { code: 'MISSION_UNAVAILABLE', message: 'That conversation could not be read from the local ledger.' } } as const
  if (typeof missionId !== 'string' || missionId.length === 0 || missionId.length > 200) return unavailable
  try {
    const recovered = await ledger.getMission(missionId)
    if (recovered === undefined) return unavailable
    const workroomMessages = new Map<string, WorkroomMessage>()
    if (workroom !== undefined) {
      try {
        for (const message of (await workroom.read()).messages) workroomMessages.set(message.messageId, message)
      } catch {
        // Unreadable peer messages show as such; the mission still opens.
      }
    }
    const mission = publicRecoveredMission(recovered, workroomMessages)
    return { ok: true, data: { mission: { ...mission, digest: missionDigest(JSON.stringify(mission)) } } }
  } catch {
    return unavailable
  }
}

/**
 * A conversation's newest turn, from any turn of it (0.391): the turns that
 * continue it, followed forward, the latest-made at each step. Through the
 * history's own cache, so it re-reads only files that changed. The turn named
 * when the ledger cannot be listed file by file.
 */
export async function newestTurnOf(ledger: MissionLedger, missionId: string): Promise<string> {
  const refreshed = await refreshedLedger(ledger)
  if (refreshed === undefined) return missionId
  const next = new Map<string, { readonly missionId: string; readonly createdAt: string }[]>()
  for (const entry of refreshed.entries) {
    const metadata = entry.light?.metadata
    const from = metadata?.continuesFrom?.missionId
    if (metadata === undefined || from === undefined) continue
    next.set(from, [...(next.get(from) ?? []), { missionId: metadata.missionId, createdAt: metadata.createdAt }])
  }
  const seen = new Set([missionId])
  let newest = missionId
  for (;;) {
    const later = (next.get(newest) ?? [])
      .filter((turn) => !seen.has(turn.missionId))
      .reduce<{ readonly missionId: string; readonly createdAt: string } | undefined>((best, turn) => (best === undefined || turn.createdAt > best.createdAt ? turn : best), undefined)
    if (later === undefined) return newest
    seen.add(later.missionId)
    newest = later.missionId
  }
}

export function withLiveMissionIds(response: MissionHistoryResponse, liveMissionIds: readonly string[]): MissionHistoryResponse {
  return response.ok ? { ...response, data: { ...response.data, liveMissionIds: [...new Set(liveMissionIds)] } } : response
}

export async function readMissionHistory(
  ledger: MissionLedger,
  workroom?: Workroom,
  /** The folder this window works in; defaults to the process's own. */
  workspacePath: string = process.cwd(),
  /** What the window already holds, `missionId -> digest` (see `missionDigest`). */
  known?: ReadonlyMap<string, string>,
  /** Where each agent's last usage reading is kept; absent, only the newest conversations are read. */
  usageFile?: string
): Promise<MissionHistoryResponse> {
  try {
    // The kept page when the ledger can give one (see CachedLedgerFile);
    // the whole listing otherwise, which is what a test's fake ledger is.
    const page: HistoryPage =
      (await cachedHistoryPage(ledger, MAX_LISTED_MISSIONS)) ??
      (await ledger.listMissions({ limit: MAX_LISTED_MISSIONS }).then((snapshot) => ({
        missions: snapshot.missions.map((mission) => ({ mission })),
        issueCount: snapshot.issues.length,
        unreadableCount: snapshot.unreadableCount
      })))
    // The workroom is joined best-effort: a channel that cannot be read makes
    // every peer message show as unreadable, which is the truthful state, and
    // must not take the mission history down with it.
    const workroomMessages = new Map<string, WorkroomMessage>()
    if (workroom !== undefined) {
      try {
        for (const message of (await workroom.read()).messages) {
          workroomMessages.set(message.messageId, message)
        }
      } catch {
        // Every link then reads as `text: null`, which is what happened.
      }
    }
    /*
     * The newest few come whole, because those are the ones about to be
     * opened. Order is the reader's: most recently touched first.
     */
    const recent = page.missions.slice(0, MAX_HISTORY_MISSIONS).map((entry) => entry.mission)
    const shown = budgeted(recent.map((mission) => publicRecoveredMission(mission, workroomMessages))).map(
      ({ mission, json }): PublicRecoveredMission => {
        const digest = missionDigest(json)
        // Unchanged since the window last had it: the window keeps its own.
        return known?.get(mission.missionId) === digest
          ? { ...mission, events: [], digest, eventsKept: true }
          : { ...mission, digest }
      }
    )
    /*
     * And every OTHER conversation as a row and nothing else.
     *
     * `events: []` is the whole trick -- the same projection the ancestor
     * walk uses, for the same reason. Opening one of these fetches its
     * transcript; until then it costs a few hundred bytes and it EXISTS,
     * which is the difference between a conversation you have not opened in
     * a while and a conversation that is gone.
     *
     * Note the slice is from `shown.length`, not from MAX_HISTORY_MISSIONS:
     * the byte budget can cut the whole page short, and anything it dropped
     * must come back here as a row rather than vanish.
     */
    const listedOnly = page.missions.slice(shown.length).map((entry) => ({
      ...publicRecoveredMission(entry.mission, workroomMessages),
      events: [],
      // A light record has no events to count; the kept numbers say it.
      ...(entry.eventCount === undefined ? {} : { eventCount: entry.eventCount }),
      ...(entry.eventsTruncated === undefined ? {} : { eventsTruncated: entry.eventsTruncated }),
      // Nor to price: the money read when its file was parsed says it.
      ...(amountsOf(entry.money) === undefined ? {} : { money: amountsOf(entry.money)! })
    }))
    const listed = [...shown, ...listedOnly]
    return {
      ok: true,
      data: {
        missions: [...listed, ...(await ancestorsOf(listed, ledger, workroomMessages))],
        currentWorkspaceId: workspaceIdFor(workspacePath),
        issueCount: page.issueCount,
        // Straight from the reader. It used to be derived here by comparing
        // issue ids against the recovered missions, which is wrong the moment
        // that list is a page rather than the whole ledger -- see
        // `unreadableFileCount` for what that cost.
        unreadableCount: page.unreadableCount,
        ...(page.totalCount !== undefined && page.totalCount > page.missions.length ? { totalMissions: page.totalCount, listedMissions: page.missions.length } : {}),
        // These two read EVENTS, so they see the missions that came with
        // events. Widening the list did not widen them: the answer is about
        // recent runs either way, and scanning three hundred transcripts to
        // decide whether a runtime is at its limit would be a new cost for no
        // new fact.
        limitedRuntimes: limitedRuntimesFrom(recent),
        usageWindows: usageFile === undefined
          ? usageWindowsFrom(recent)
          : await rememberedUsageWindows(usageWindowsFrom(recent), usageFile, async () => {
            const back: RecoveredMission[] = []
            for (const entry of page.missions.slice(MAX_HISTORY_MISSIONS, USAGE_BACKFILL_MISSIONS)) {
              const whole = await ledger.getMission(entry.mission.metadata.missionId).catch(() => undefined)
              if (whole !== undefined) back.push(whole)
            }
            return back
          })
      }
    }
  } catch {
    return {
      ok: false,
      error: {
        code: 'HISTORY_UNAVAILABLE',
        message: 'Local mission history could not be read.'
      }
    }
  }
}

/**
 * Delete a mission's record, unless it is still running.
 *
 * Deleting a live mission would orphan a process that keeps writing into a
 * file that no longer exists -- and it would take the person's only stop
 * control with it. So the answer is a refusal that names the remedy. Nothing
 * here touches the workroom: a deleted mission's messages stay, attributed,
 * because the person they were sent to still has a right to see them.
 */
export async function deleteMissionRecord(
  ledger: MissionLedger,
  missionId: unknown,
  isLive: (missionId: string) => boolean
): Promise<MissionDeleteResponse> {
  if (typeof missionId !== 'string' || !/^[A-Za-z0-9_-]{1,128}$/.test(missionId)) {
    return { ok: false, error: { code: 'NOT_FOUND', message: 'That mission does not exist.' } }
  }
  if (isLive(missionId)) {
    return {
      ok: false,
      error: { code: 'LIVE', message: 'That mission is still running. Stop it first, then delete it.' }
    }
  }
  try {
    const removed = await ledger.deleteMission(missionId)
    return removed
      ? { ok: true }
      : { ok: false, error: { code: 'NOT_FOUND', message: 'That mission does not exist.' } }
  } catch {
    return { ok: false, error: { code: 'INTERNAL_ERROR', message: 'The mission could not be deleted.' } }
  }
}
