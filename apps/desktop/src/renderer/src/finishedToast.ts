import { openQuestion } from './needsYou.js'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

/**
 * "WREN FINISHED" -- WHEN A FINISH IS WORTH HEARING ABOUT (0.379).
 *
 * attention.ts never toasted a finished run, on purpose: "a notification
 * for every finished run would train a person to dismiss the one that
 * matters." Orca notifies on every done (read 2026-09-26), and softens it
 * with cooldowns. This keeps the rule's point and answers the case it left
 * out -- a person who set a teammate on a long job and went to another app:
 *
 * - only a run the person started here (not a room member, whose room
 *   gathers its own toasts, nor a relay, a routine, or a run restored from
 *   the record);
 * - only one that took at least a minute -- a reply you were watching
 *   arrive needs no toast;
 * - finished or failed, never stopped: whoever pressed Stop knows;
 * - and only when the window is elsewhere, which the host decides.
 */

/** Below this, the person was very likely still watching. */
export const FINISH_WORTH_TELLING_MS = 60_000
const MAX_LINE = 140

export interface EndedRun {
  readonly phase: string
  readonly startedAtIso: string | undefined
  readonly endedAtMs: number
  /** Who started it; undefined is the person. */
  readonly startedBy: unknown
  readonly restored: boolean
  readonly teammateName: string | undefined
  readonly events: readonly NormalizedRuntimeEvent[]
  readonly error: string | undefined
}

/** The first line of what the run last said in full, bounded to a toast. */
export function lastSaid(events: readonly NormalizedRuntimeEvent[]): string | undefined {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index]!
    if (event.type !== 'message.delta') continue
    const payload = event.payload as { readonly final?: boolean; readonly text?: string }
    if (payload.final !== true || typeof payload.text !== 'string') continue
    const line = payload.text.split('\n').map((part) => part.trim()).find((part) => part.length > 0)
    if (line === undefined) continue
    return line.length > MAX_LINE ? `${line.slice(0, MAX_LINE - 1).trimEnd()}…` : line
  }
  return undefined
}

function tookText(ms: number): string {
  const minutes = Math.max(1, Math.round(ms / 60_000))
  return minutes < 60 ? `${String(minutes)} min` : `${String(Math.floor(minutes / 60))} h ${String(minutes % 60)} min`
}

/** The toast for a run that just ended, or nothing when it is not worth one. */
export function finishedToast(run: EndedRun): { readonly title: string; readonly body: string } | undefined {
  if (openQuestion(run.events) !== undefined) return undefined
  if (run.restored || run.startedBy !== undefined) return undefined
  if (run.phase !== 'completed' && run.phase !== 'failed') return undefined
  if (run.startedAtIso === undefined) return undefined
  const took = run.endedAtMs - Date.parse(run.startedAtIso)
  if (!Number.isFinite(took) || took < FINISH_WORTH_TELLING_MS) return undefined
  const who = run.teammateName ?? 'A teammate'
  if (run.phase === 'failed') {
    const why = (run.error ?? '').split('\n')[0]?.trim() ?? ''
    return { title: `${who} stopped`, body: `${why.length > 0 ? (why.length > MAX_LINE ? `${why.slice(0, MAX_LINE - 1).trimEnd()}…` : why) : 'The run did not finish.'} · after ${tookText(took)}` }
  }
  return { title: `${who} finished`, body: `${lastSaid(run.events) ?? 'Done.'} · took ${tookText(took)}` }
}
