/**
 * SINCE YOU WERE AWAY (0.590, the PRD's R17 / U4).
 *
 * Routines are what the buyer bought, and the morning-after view did not
 * exist: a run that finished, failed or was missed while nobody was at the
 * window was findable only by reading the sidebar row by row. This reduces
 * what the window already holds -- the conversation rows, the routines with
 * their misses, the set of turns waiting on the person -- to one list for
 * Home and one line for the tray: ran, failed, missed, waiting on you.
 *
 * `since` is the moment the person was last at the window (main/away.ts
 * keeps it). Pure, shared: the renderer reduces, the main process words the
 * tray line from the counts the renderer hands back.
 */

/** A conversation as the sidebar lists it; only what the reduction reads. */
export interface AwayRow {
  readonly missionId: string
  readonly title: string
  readonly phase: string
  /** The last moment anything happened in it; for a finished run, its end. */
  readonly lastAt?: string
  readonly ownerId?: string
  readonly routineId?: string
  readonly memberIds?: readonly string[]
  readonly rootId?: string
}

export interface AwayRoutine {
  readonly routineId: string
  readonly name: string
  readonly history?: readonly { readonly kind: 'missed'; readonly dueAt: string; readonly recordedAt: string; readonly hookCalls?: number }[]
}

export interface AwayItem {
  readonly missionId: string
  readonly title: string
  /** When it ended. */
  readonly at: string
  readonly ownerId?: string
  readonly routineId?: string
}

export interface AwayMissed {
  readonly routineId: string
  readonly name: string
  readonly dueAt: string
}

export interface AwaySummary {
  readonly since: string
  /** Newest first. */
  readonly ran: readonly AwayItem[]
  readonly failed: readonly AwayItem[]
  /** Oldest first, as a morning reads. */
  readonly missed: readonly AwayMissed[]
  /** Conversations waiting on the person right now, whenever they started. */
  readonly waiting: number
}

/** The counts alone: what the renderer hands the main process for the tray. */
export interface AwaySummaryCounts {
  readonly ran: number
  readonly failed: number
  readonly missed: number
  readonly waiting: number
}

/** Shorter than this and nobody was "away": a coffee is not a morning. */
export const AWAY_MINIMUM_MS = 20 * 60_000

const time = (value: string | undefined): number => {
  const parsed = value === undefined ? Number.NaN : Date.parse(value)
  return Number.isFinite(parsed) ? parsed : Number.NaN
}

export function sinceYouWereAway(input: {
  readonly since: string
  readonly rows: readonly AwayRow[]
  readonly routines: readonly AwayRoutine[]
  readonly needsYou: ReadonlySet<string>
}): AwaySummary | undefined {
  const since = time(input.since)
  if (!Number.isFinite(since)) return undefined
  const ended = (row: AwayRow): AwayItem | undefined => {
    const at = time(row.lastAt)
    if (!Number.isFinite(at) || at <= since) return undefined
    return {
      missionId: row.missionId,
      title: row.title,
      at: row.lastAt!,
      ...(row.ownerId === undefined ? {} : { ownerId: row.ownerId }),
      ...(row.routineId === undefined ? {} : { routineId: row.routineId })
    }
  }
  const newestFirst = (left: AwayItem, right: AwayItem): number => time(right.at) - time(left.at)
  const ran = input.rows.filter((row) => row.phase === 'completed').map(ended).filter((item): item is AwayItem => item !== undefined).sort(newestFirst)
  const failed = input.rows.filter((row) => row.phase === 'failed').map(ended).filter((item): item is AwayItem => item !== undefined).sort(newestFirst)
  const waiting = input.needsYou.size === 0
    ? 0
    : input.rows.filter((row) => [row.missionId, row.rootId, ...(row.memberIds ?? [])].some((id) => id !== undefined && input.needsYou.has(id))).length
  const seen = new Set<string>()
  const missed: AwayMissed[] = []
  for (const routine of input.routines) {
    for (const entry of routine.history ?? []) {
      if (entry.kind !== 'missed' || entry.hookCalls !== undefined) continue
      // A miss is news when it was RECORDED after the person left: a slot that
      // passed while they were away, however old the slot itself reads.
      if (!(time(entry.recordedAt) > since || time(entry.dueAt) > since)) continue
      const key = `${routine.routineId}\n${entry.dueAt}`
      if (seen.has(key)) continue
      seen.add(key)
      missed.push({ routineId: routine.routineId, name: routine.name, dueAt: entry.dueAt })
    }
  }
  missed.sort((left, right) => time(left.dueAt) - time(right.dueAt))
  if (ran.length === 0 && failed.length === 0 && missed.length === 0 && waiting === 0) return undefined
  return { since: input.since, ran, failed, missed, waiting }
}

/** The counts the tray and the heading say, parts that are zero left out. */
export function awayCounts(summary: AwaySummary): AwaySummaryCounts {
  return { ran: summary.ran.length, failed: summary.failed.length, missed: summary.missed.length, waiting: summary.waiting }
}

/**
 * "Since you were away: 2 ran · 1 failed · 1 missed · 1 waiting on you".
 * Undefined when every count is zero (nothing to say, so no line).
 */
export function awayLine(counts: AwaySummaryCounts | null | undefined): string | undefined {
  if (counts === null || counts === undefined) return undefined
  const parts = [
    counts.ran > 0 ? `${String(counts.ran)} ran` : undefined,
    counts.failed > 0 ? `${String(counts.failed)} failed` : undefined,
    counts.missed > 0 ? `${String(counts.missed)} missed` : undefined,
    counts.waiting > 0 ? `${String(counts.waiting)} waiting on you` : undefined
  ].filter((part): part is string => part !== undefined)
  if (parts.length === 0) return undefined
  return `Since you were away: ${parts.join(' · ')}`
}

/** The counts as they arrive over IPC: four whole numbers, or null for nothing to say. */
export function awayCountsFrom(value: unknown): AwaySummaryCounts | null {
  if (typeof value !== 'object' || value === null) return null
  const record = value as Record<string, unknown>
  const count = (name: string): number | undefined => {
    const held = record[name]
    return typeof held === 'number' && Number.isInteger(held) && held >= 0 && held <= 1_000_000 ? held : undefined
  }
  const ran = count('ran')
  const failed = count('failed')
  const missed = count('missed')
  const waiting = count('waiting')
  if (ran === undefined || failed === undefined || missed === undefined || waiting === undefined) return null
  if (ran + failed + missed + waiting === 0) return null
  return { ran, failed, missed, waiting }
}

/** Whether a gap in attention counts as having been away. */
export function wasAway(lastAt: string | undefined, now: Date, minimum = AWAY_MINIMUM_MS): string | undefined {
  const at = time(lastAt)
  if (!Number.isFinite(at)) return undefined
  return now.getTime() - at >= minimum ? lastAt : undefined
}
