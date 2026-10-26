/**
 * How far each runtime's integration goes, readable by BOTH processes.
 *
 * It lived in the renderer's status.ts, where the main process could not
 * read it -- and on 2026-09-22 the main process needed it: discovery asks
 * each runtime whether it is signed in, and for a runtime Locust lists but
 * cannot run (`planned`) that answer is never used. Gemini CLI's check was
 * the slowest of every launch (3.2 s warm, 7.4 s cold). One list, so the
 * loading code and the screens cannot disagree about what is planned.
 */
/** How far a runtime's integration actually goes in this build. */
export type IntegrationLevel =
  /** Can own a live mission end to end today. */
  | 'live'
  /** Discovered and selectable, but the adapter is not finished. */
  | 'preview'
  /**
   * Runs a mission end to end, but through a reverse-engineered surface
   * that its vendor did not publish and may change without notice. Said on
   * the row, so nobody mistakes it for a supported route.
   */
  | 'experimental'
  /** Drawn in the design, not implemented at all. */
  | 'planned'

/**
 * How far each integration actually goes, in ONE place.
 *
 * This lived in three files -- the route picker, Settings and the first-launch
 * panel -- and drifted the moment a runtime was added. On 2026-09-02 the same
 * screen showed Cursor Agent as READY in Settings and PLANNED in the welcome
 * panel, which is precisely the "you can always tell what is really in play"
 * claim failing at the only moment a newcomer looks. A new runtime is now one
 * edit, not three.
 */
export const RUNTIME_INTEGRATION: Readonly<Record<string, IntegrationLevel>> = {
  codex: 'live',
  claude: 'live',
  cursor: 'live',
  opencode: 'live',
  copilot: 'live',
  antigravity: 'experimental',
  /*
   * Muse Code owns a mission end to end and its runs are as durable as any
   * other -- measured twice through the real builder, runner and normalizer
   * on the free echo provider. It was PREVIEW until a run under a paying
   * provider had been seen with real tool calls.
   * LIVE SINCE 2026-10-25: two missions in a person's own ledger on Muse
   * Wonder 1.0 (2026-10-19 and 2026-10-22), each a whole run.completed with
   * 60-odd tool calls through the same rows (shell, read, edit, write,
   * search, to-dos) and the odd tool.failed shown as one.
   */
  muse: 'live',
  gemini: 'planned',
  omniroute: 'planned'
}

/** What a runtime this build does not know should be treated as. */
export function integrationOf(runtimeId: string): IntegrationLevel {
  return RUNTIME_INTEGRATION[runtimeId] ?? 'planned'
}

/** The runtimes this build lists but cannot start a mission on. */
export function plannedRuntimes(): ReadonlySet<string> {
  return new Set(Object.entries(RUNTIME_INTEGRATION).filter(([, level]) => level === 'planned').map(([id]) => id))
}
