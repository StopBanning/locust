import type { RoutineHandOff, TeammateRoute } from './ipc.js'

/** A dispatch receipt, not a claim that the runtime did (or did not) do work. */
/**
 * H2: a routine whose last attempt is not settled -- dispatching, running,
 * or held for the person's review -- is not removed out from under that
 * review; the review card's Abandon, behind its checkbox, is the way out.
 */
export function routineAwaitsReview(routine: { readonly execution?: RoutineExecution }): boolean {
  const status = routine.execution?.status
  return status !== undefined && status !== 'abandoned'
}

export interface RoutineExecution {
  /** A called link keeps every step in Ask, including steps handed to another teammate. */
  readonly hook?: true
  readonly attemptId: string
  readonly status: 'dispatching' | 'running' | 'held' | 'abandoned'
  readonly step: number
  readonly of: number
  readonly startedAt: string
  readonly updatedAt: string
  readonly steps: readonly string[]
  readonly route: TeammateRoute
  /** Who took each step, as the attempt started with it (0.435). */
  readonly handOffs?: readonly RoutineHandOff[]
  /** Its steps run in the routine's copy, as the attempt started (0.533). */
  readonly inCopy?: true
  /** Its standing goal, as the attempt started (0.534). */
  readonly untilCheck?: { readonly tries: number }
  /** How many fixes it has been asked for since its steps were done (0.534). */
  readonly goalTry?: number
  readonly workspaceId: string
  readonly recovered?: boolean
  readonly missionId?: string
  readonly runId?: string
  readonly followUpOf?: string
  readonly reason?: string
  readonly canContinue?: boolean
  /**
   * The dispatch itself answered, and the answer will not change.
   *
   * A hold written when the runtime REFUSED the start names the refusal --
   * "Cursor Agent cannot be held read-only on this system", say -- and that
   * is the most useful sentence this attempt will ever have. Reconciliation
   * re-decides every held attempt, and with no mission to ask about it
   * rewrote that sentence as "the app stopped before saving a mission
   * receipt": a crash that had not happened, in place of the one fact that
   * told the person what to change. Colin, 2026-09-21: *"bug?"*
   *
   * So a hold decided AT DISPATCH says so, and reconciliation leaves it
   * alone. Nothing later can teach it anything: there is no mission whose
   * phase could move.
   */
  readonly settledAtDispatch?: boolean
}

export const ROUTINE_RECOVERY_CHANNEL = 'routine:recovery'
export interface RoutineRecoveryRequest {
  readonly routineId: string
  readonly attemptId: string
  readonly step: number
  /**
   * `keep` (0.392): the person reviewed the attempt and wants the routine to
   * go on as it was -- the attempt is cleared, the schedule stays, and the
   * next run starts from step 1 when it is next due. The 0.390 beta pass
   * found no way out of an interrupted LAST step but Abandon, which also
   * removes the schedule.
   */
  readonly decision: 'continue' | 'abandon' | 'keep'
}
export type RoutineRecoveryResponse =
  | { readonly ok: true }
  | { readonly ok: false; readonly error: { readonly message: string } }

export function isRoutineRecoveryRequest(value: unknown): value is RoutineRecoveryRequest {
  if (typeof value !== 'object' || value === null) return false
  const input = value as Record<string, unknown>
  return typeof input.routineId === 'string' && /^[\w-]{1,100}$/.test(input.routineId)
    && typeof input.attemptId === 'string' && /^[\w-]{1,100}$/.test(input.attemptId)
    && Number.isInteger(input.step) && Number(input.step) >= 1 && Number(input.step) <= 12
    && (input.decision === 'continue' || input.decision === 'abandon' || input.decision === 'keep')
}
