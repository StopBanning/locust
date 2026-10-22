import { CHECK_FIX_STOPPED, checkFixPrompt, type CheckFixStarter } from '../shared/check-fix.js'
import type { EditCheckResult } from './edit-check.js'

/**
 * FIX UNTIL THE CHECK PASSES (0.812, shared/check-fix.ts): whether a failed check goes back to the teammate.
 *
 * Called when the person's check has said how it ended after a turn (codex-mission.ts, after `edit-check`). It sends
 * the failure back -- the next turn of the same conversation, on the same route, recorded as Locust's (`check-fix`) --
 * only when every one of these holds:
 *   - the person turned it on for this folder, with a most;
 *   - the check failed with something new (a failure that was already there is not this turn's doing);
 *   - the turn finished on its own, and the person sent it, or it was itself a fix;
 *   - it is not a comparison's column, a side question or a reply between teammates;
 *   - nobody is already talking to the teammate in it;
 *   - fixes are left.
 * When the setting is on but one of the last three stops it, the thread says why in one line, under that turn; a
 * failure the turn did not cause needs no line, since the check's own card says nothing in it is new.
 *
 * A fix after a fix is judged against what was failing BEFORE the person's turn, not against the check just before
 * it: a fix that clears two of three errors leaves nothing new, yet it moved; and one that clears everything the turn
 * broke has done its work even while an older failure stands.
 */

/** The turn a check ran after, as the mission service knew it when the turn ended. */
export interface CheckedTurn {
  readonly missionId: string
  /** Whose turn it was; a conversation with nobody has none. */
  readonly teammateId?: string
  /** It finished on its own: not stopped, not failed. */
  readonly completed: boolean
  /** A comparison's column, a side question or a reply between teammates: never sent back. */
  readonly apart: boolean
  readonly startedBy?: { readonly kind: string; readonly fix?: number }
}

export type CheckFixStart =
  | { readonly ok: true; readonly missionId: string }
  | { readonly ok: false; readonly message: string }

export interface CheckFixOptions {
  /** This folder's most fixes, read when the check ends: undefined while the person has not turned it on. */
  readonly mostFixes: () => Promise<number | undefined>
  /** Whether the person sent this turn, from the chat box or the phone. */
  readonly sentByPerson: (missionId: string) => boolean
  /** Whether the conversation has moved on or its teammate is already working: the person got there first. */
  readonly busy: (turn: CheckedTurn) => Promise<boolean>
  readonly nameOf: (teammateId: string | undefined) => Promise<string>
  /** The fix: the next turn of the same conversation, on the turn's own route and in its own mode. */
  readonly start: (input: {
    readonly followUpOf: string
    readonly teammateId?: string
    readonly prompt: string
    readonly startedBy: CheckFixStarter
  }) => Promise<CheckFixStart>
  /**
   * One line in the thread, under the turn it is about: `info` when there was nothing for a fix to do, `warning` when
   * the check still fails and the person may need to act.
   */
  readonly note: (missionId: string, message: string, level: 'info' | 'warning') => void
}

export interface CheckFix {
  /** `no-edits`: a fix turn that changed no files, so its check was not run again. */
  onChecked(turn: CheckedTurn, result: EditCheckResult | 'no-edits'): Promise<void>
}

/** Turns already sent back, so a second word about the same check never starts a second fix; the oldest go first. */
const REMEMBERED = 500
/** Fixes under way, by the fix turn's mission. */
const CHAINS = 100
/** As much of a failure as a fix is told, the way edit-check bounds what it shows (MAX_NEW_LINES, MAX_LINE). */
const TOLD_LINES = 30
const TOLD_LINE = 300

/** What a run of fixes started from, carried from one fix to the next. */
interface Chain {
  /** What the check printed before the person's turn: never that turn's doing, so never sent back. */
  readonly baseline: ReadonlySet<string>
  /** What it printed beyond that the last time it ran: what the fix was asked to fix. */
  readonly caused: ReadonlySet<string>
  /** There was no check before the person's turn, so every line was the turn's to answer for. */
  readonly first: boolean
}

const told = (lines: readonly string[]): readonly string[] =>
  lines.slice(-TOLD_LINES).map((line) => (line.length > TOLD_LINE ? `${line.slice(0, TOLD_LINE - 1)}…` : line))

const sameLines = (lines: readonly string[], held: ReadonlySet<string>): boolean =>
  lines.length === held.size && lines.every((line) => held.has(line))

export function createCheckFix(options: CheckFixOptions): CheckFix {
  const sent = new Set<string>()
  const chains = new Map<string, Chain>()
  const bounded = <T>(held: Set<T> | Map<T, unknown>, most: number): void => {
    while (held.size > most) {
      const oldest = held.keys().next().value
      if (oldest === undefined) break
      held.delete(oldest)
    }
  }
  // One word about a turn at a time: a second while the first is still deciding is about the same check.
  const deciding = new Set<string>()
  const decide = async (turn: CheckedTurn, result: EditCheckResult | 'no-edits'): Promise<void> => {
    if (!turn.completed || turn.apart) return
    // Fixes so far: none for a turn the person sent; a fix's own number for a fix. Anything else is not ours.
    const done = turn.startedBy?.kind === 'check-fix'
      ? turn.startedBy.fix
      : turn.startedBy === undefined && options.sentByPerson(turn.missionId) ? 0 : undefined
    if (done === undefined || !Number.isSafeInteger(done) || done < 0) return
    const chain = done === 0 ? undefined : chains.get(turn.missionId)
    chains.delete(turn.missionId)
    if (result !== 'no-edits' && result.outcome !== 'failed') return
    const most = await options.mostFixes().catch(() => undefined)
    if (most === undefined || most < 1) return
    if (result === 'no-edits') {
      if (done > 0) options.note(turn.missionId, CHECK_FIX_STOPPED.changedNothing(done), 'warning')
      return
    }
    // What the check says now beyond what was failing before the person's turn.
    const baseline = chain?.baseline ?? new Set(result.before ?? [])
    const caused = (result.lines ?? result.newLines).filter((line) => !baseline.has(line))
    if (done === 0 || chain === undefined) {
      // Judged against the check before this turn: for a fix, only when Locust has restarted since the chain began.
      // After the person's own turn, nothing is said: the card under it already says nothing in the failure is new.
      if (result.unchanged) {
        if (done > 0) options.note(turn.missionId, CHECK_FIX_STOPPED.noProgress(done), 'warning')
        return
      }
    } else if (caused.length === 0) {
      options.note(turn.missionId, CHECK_FIX_STOPPED.fixedWhatItBroke(done), 'info')
      return
    } else if (sameLines(caused, chain.caused)) {
      options.note(turn.missionId, CHECK_FIX_STOPPED.noProgress(done), 'warning')
      return
    }
    if (done >= most) {
      options.note(turn.missionId, CHECK_FIX_STOPPED.outOfFixes(done), 'warning')
      return
    }
    if (await options.busy(turn).catch(() => true)) {
      options.note(turn.missionId, CHECK_FIX_STOPPED.busy(await options.nameOf(turn.teammateId).catch(() => 'the teammate')), 'info')
      return
    }
    // Taken before the start, so a word about this check after the fix has begun is not run twice.
    sent.add(turn.missionId)
    bounded(sent, REMEMBERED)
    const fix = done + 1
    const first = chain?.first ?? result.first
    const startedBy: CheckFixStarter = { kind: 'check-fix', fix, of: most, command: result.command }
    let started: CheckFixStart
    try {
      started = await options.start({
        followUpOf: turn.missionId,
        ...(turn.teammateId === undefined ? {} : { teammateId: turn.teammateId }),
        prompt: checkFixPrompt({ command: result.command, lines: told(caused), first }, fix, most),
        startedBy
      })
    } catch {
      started = { ok: false, message: 'The turn could not be started.' }
    }
    if (!started.ok) {
      sent.delete(turn.missionId)
      options.note(turn.missionId, CHECK_FIX_STOPPED.couldNotStart(started.message), 'warning')
      return
    }
    chains.set(started.missionId, { baseline, caused: new Set(caused), first })
    bounded(chains, CHAINS)
  }
  return {
    async onChecked(turn, result) {
      if (deciding.has(turn.missionId) || sent.has(turn.missionId)) return
      deciding.add(turn.missionId)
      try {
        await decide(turn, result)
      } finally {
        deciding.delete(turn.missionId)
      }
    }
  }
}
