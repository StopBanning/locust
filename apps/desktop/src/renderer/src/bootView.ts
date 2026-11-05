import type { DiscoveryEvent } from '../../shared/ipc.js'
import { integrationOf } from './status.js'

/**
 * The boot screen, as a value.
 *
 * Kept out of the component on purpose: the interesting parts of this screen
 * are a phase machine and a set of timings, and neither needs a DOM to be
 * wrong. Everything here is derived from real discovery events — nothing is
 * invented except the one pause named below, and a pause is not a fact.
 */

export type BootPhase = 'idle' | 'probing' | 'settling' | 'settled' | 'dissolving' | 'gone'

/**
 * The beat before the settle, so the screen does not flinch at the last result.
 *
 * SHORTER SINCE 2026-09-22. Measured end to end on Colin's machine, the
 * ceremony after the last runtime answered was 1.96 s of a 6.7 s launch --
 * the single largest fixed cost once the runtimes were found. Colin, the
 * same day: "i want the users to have a seamless, fast experience". The
 * settle and the dissolve are the animations and keep their lengths; the
 * pause before (380 -> 200) and the hold after (900 -> 350) were waiting.
 */
export const SETTLE_DELAY_MS = 200
export const SETTLE_MS = 420
export const SETTLED_HOLD_MS = 350
export const DISSOLVE_MS = 260
/** Past this a counter stops being reassurance and starts being a worry. */
export const STALLED_AFTER_MS = 6_000

export interface BootRow {
  readonly bin: string
  readonly product: string
  readonly startedAt: number
  readonly result?: string
  readonly tone: 'ready' | 'needs-signin' | 'missing' | 'error' | 'pending'
  readonly elapsed: string
  readonly stalled: boolean
}

export interface BootPre {
  readonly key: string
  readonly value: string
  readonly tag?: string
  readonly tone: 'lime' | 'green' | 'plain'
}

export interface BootView {
  readonly phase: BootPhase
  readonly preamble: readonly BootPre[]
  readonly rows: readonly BootRow[]
  readonly progress: string
  readonly summary: string
  readonly allAnswered: boolean
}

export interface BootState {
  readonly phase: BootPhase
  readonly startedAt?: number
  readonly context?: Extract<DiscoveryEvent, { kind: 'context' }>
  readonly probes: readonly {
    readonly id: string
    readonly bin: string
    readonly product: string
    readonly at: number
    readonly outcome?: 'missing' | 'needs-signin' | 'ready' | 'error'
    readonly version?: string
  }[]
  readonly finished?: { readonly at: number; readonly ready: number; readonly needsYou: number }
}

export const emptyBoot: BootState = { phase: 'idle', probes: [] }

/**
 * Fold one discovery event into the screen's state.
 *
 * `started` resets everything, because a second sweep is a second log and a
 * screen showing ten probes for five runtimes is showing neither.
 */
export function applyDiscoveryEvent(state: BootState, event: DiscoveryEvent): BootState {
  switch (event.kind) {
    case 'started':
      // `context` is about the launch, not the sweep, so it survives a
      // restart the way it does in the host's own log.
      return { ...state, phase: 'probing', startedAt: event.at, probes: [] }
    case 'context':
      return { ...state, context: event }
    case 'probe.started':
      // Keyed by ID, not by the command name: those differ for Cursor, and
      // keying on the wrong one left its result unable to find its row.
      // A repeat replaces rather than appends -- one row per runtime is the
      // whole shape of the log.
      return {
        ...state,
        phase: state.phase === 'idle' ? 'probing' : state.phase,
        probes: [
          ...state.probes.filter((probe) => probe.id !== event.id),
          { id: event.id, bin: event.bin, product: event.product, at: event.at }
        ]
      }
    case 'probe.finished':
      return {
        ...state,
        probes: state.probes.map((probe) =>
          probe.id === event.id
            ? { ...probe, outcome: event.outcome, ...(event.version === undefined ? {} : { version: event.version }) }
            : probe
        )
      }
    case 'finished':
      return { ...state, finished: { at: event.at, ready: event.ready, needsYou: event.needsYou } }
    default:
      return state
  }
}

const TONE: Record<string, BootRow['tone']> = {
  ready: 'ready',
  'needs-signin': 'needs-signin',
  missing: 'missing',
  error: 'error'
}

/**
 * A path a person can read at a glance.
 *
 * The ledger lives under the user's own folder, and spelling that out in
 * full is both long enough to run off the screen and a needless printing of
 * somebody's name. `~` is what a terminal would show, which is what this is
 * dressed as.
 */
export function homeRelative(path: string): string {
  const separator = path.includes('/') && !path.includes(String.fromCharCode(92)) ? '/' : String.fromCharCode(92)
  const parts = path.split(separator)
  const users = parts.findIndex((part) => part === 'Users' || part === 'home')
  // `Users/<name>/...` -- drop the drive, the folder and the name.
  return users >= 0 && parts.length > users + 2
    ? ['~', ...parts.slice(users + 2)].join(separator)
    : path
}

function resultText(outcome: string, version: string | undefined): string {
  // Not "not installed" (0.867): the words an error uses, and the ones 0.866 took off Home for an agent not added yet.
  if (outcome === 'missing') return 'ready to install'
  if (outcome === 'needs-signin') return 'sign-in required'
  if (outcome === 'error') return 'did not answer'
  return version === undefined ? 'ready' : `ready · ${version}`
}

/**
 * What a person should read right now.
 *
 * `now` is passed in rather than read, so the elapsed counters and the
 * stalled threshold can be tested at an exact moment instead of by waiting.
 */
export function bootView(state: BootState, phase: BootPhase, now: number): BootView {
  const preamble: BootPre[] = []
  const context = state.context
  if (context !== undefined) {
    preamble.push({ key: 'locust', value: `${context.version} · ${context.platform}`, tone: 'lime' })
    preamble.push({
      key: 'workspace',
      value: context.branch === undefined ? context.workspace : `${context.workspace} · git ${context.branch}`,
      tone: 'plain',
      ...(context.clean === undefined ? {} : { tag: context.clean ? 'clean' : 'uncommitted' })
    })
    preamble.push({
      key: 'ledger',
      value: homeRelative(context.ledgerPath),
      tone: context.ledgerOk ? 'green' : 'plain',
      tag: context.ledgerOk ? 'ok' : 'not writable'
    })
  }

  /*
   * THE LOG LISTS WHAT THIS BUILD CAN USE.
   *
   * Discovery probes every definition, roadmap ones included, and the boot
   * screen was printing their results: Colin's launch showed `gemini ...
   * sign-in required` while Settings, two clicks away, said "Not built yet.
   * Shown so the roadmap is visible, not because it works." Both cannot be
   * true, and the probe result is the one that misleads -- there is nothing
   * to sign in to.
   *
   * Filtered here rather than at the probe, because discovery still wants
   * to know what is on the machine; it is this SCREEN that should not offer
   * a fact about something the app cannot run.
   */
  const rows: BootRow[] = state.probes.filter((probe) => integrationOf(probe.id) !== 'planned').map((probe) => {
    const waited = Math.max(0, now - probe.at)
    return {
      bin: probe.bin,
      product: probe.product,
      startedAt: probe.at,
      ...(probe.outcome === undefined ? {} : { result: resultText(probe.outcome, probe.version) }),
      tone: probe.outcome === undefined ? 'pending' : (TONE[probe.outcome] ?? 'error'),
      elapsed: `${(waited / 1000).toFixed(1)}s`,
      stalled: probe.outcome === undefined && waited >= STALLED_AFTER_MS
    }
  })

  // AFTER the rows, and counting THEM: it said "8 so far" above six rows,
  // because it was counting every probe including the roadmap ones the list
  // does not show (Colin's launch, 2026-09-14).
  if (rows.length > 0) {
    preamble.push({ key: 'scanning', value: 'PATH', tone: 'plain', tag: `${String(rows.length)} so far` })
  }

  const answered = rows.filter((row) => row.result !== undefined).length
  const finished = state.finished
  const summary =
    finished === undefined
      ? ''
      : `${String(finished.ready)} ready · ${String(finished.needsYou)} needs you · nothing pooled, proxied, or sent anywhere you have not connected`

  return {
    phase,
    preamble,
    rows,
    /*
     * `checking 0 of 0 runtimes` is arithmetic about an empty set, and on a
     * bare machine it is the ONLY frame anyone sees: a probe for a binary
     * that is not on PATH answers in about 10ms, so the whole ceremony is
     * over in half a second and this is the line it flashes.
     *
     * Grok's pass 4, 2026-09-15, driving exactly that machine: "The readout
     * in that flash is empty arithmetic, not reassurance... if the splash is
     * going to exist for Ian at all, `checking 0 of 0` is the wrong line;
     * the welcome already has the right one."
     *
     * So when there is nothing to count, say what is true instead of
     * counting it. The welcome behind this says the rest.
     */
    progress:
      rows.length === 0
        ? 'looking for AI agents on this machine '
        : `checking ${String(answered)} of ${String(rows.length)} AI agents `,
    summary,
    allAnswered: rows.length > 0 && answered === rows.length
  }
}

/**
 * How long after `finished` each phase begins.
 *
 * The 380ms beat before settling is the one invented duration in the
 * sequence, and it is a pause rather than a fabrication: settling on the
 * instant the last result lands reads as the screen flinching.
 *
 * There is NO minimum dwell. If discovery takes 200ms the person sees a
 * flash and is straight into the app.
 */
export function phaseAt(finishedAt: number, now: number): BootPhase {
  const since = now - finishedAt
  if (since < SETTLE_DELAY_MS) return 'probing'
  if (since < SETTLE_DELAY_MS + SETTLE_MS) return 'settling'
  if (since < SETTLE_DELAY_MS + SETTLE_MS + SETTLED_HOLD_MS) return 'settled'
  if (since < SETTLE_DELAY_MS + SETTLE_MS + SETTLED_HOLD_MS + DISSOLVE_MS) return 'dissolving'
  return 'gone'
}
