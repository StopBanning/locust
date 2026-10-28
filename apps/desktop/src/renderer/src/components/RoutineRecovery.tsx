import { useState } from 'react'
import type { ReactElement } from 'react'
import type { PublicRoutine } from '../../../shared/ipc.js'
import type { RoutineRecoveryRequest, RoutineRecoveryResponse } from '../../../shared/routine-recovery.js'
import { nextRunAfter } from '../../../shared/routine-schedule.js'

export type RecoverRoutine = (request: RoutineRecoveryRequest) => Promise<RoutineRecoveryResponse>

/** This is durable card content, not a dismissible toast: missing one event
 * must not hide a routine that has been waiting for four days. */
export function RoutineRecovery({ routine, recover, onOpenMission, now = new Date() }: {
  readonly routine: PublicRoutine
  readonly recover?: RecoverRoutine
  readonly onOpenMission?: (missionId: string) => void
  /** The clock the next run is read from; tests pass one. */
  readonly now?: Date
}): ReactElement | null {
  const [busy, setBusy] = useState(false)
  const [reviewed, setReviewed] = useState(false)
  const [error, setError] = useState<string>()
  const execution = routine.execution
  if (execution === undefined || execution.status === 'running') return null
  // Keep starts the clock now; a once whose time has passed has no next run.
  const nextAfterKeep = routine.schedule === undefined ? undefined : nextRunAfter(routine.schedule, now.toISOString(), now)
  // A wait the routine asked for (0.840) is answered without the recovery's review box: nothing went wrong.
  const asked = execution.status === 'held' && execution.waitsForOk === true
  const decide = async (decision: RoutineRecoveryRequest['decision']): Promise<void> => {
    if (recover === undefined || busy || (!reviewed && !asked)) return
    setBusy(true)
    setError(undefined)
    try {
      const response = await recover({ routineId: routine.routineId, attemptId: execution.attemptId, step: execution.step, decision })
      if (!response.ok) setError(response.error.message)
    } catch {
      setError('Could not record your decision. Reload routines and check the saved attempt before trying again.')
    } finally {
      setBusy(false)
      setReviewed(false)
    }
  }
  /*
   * A CARD OF ITS OWN, laid out as the lines it is.
   *
   * It wore the row's `lc-routinerow__meta` class as well, and the Routines
   * screen lays a row's meta out as ONE flex line in ONE grid cell, clipped
   * -- so every sentence here became a column a few words wide, and the
   * review box and both decisions were cut off where nobody could reach them
   * (Colin's screenshot from testing, 2026-09-27). The Team screen's card was
   * never given that class's rules, which is why it looked right there.
   *
   * Amber is right and stays: this holds controls and is waiting on a person,
   * which is the pending register -- the head says it in amber, on an amber
   * edge, and the sentences under it are read in the ordinary ink.
   */
  const nextRun = nextAfterKeep === undefined
    ? (routine.schedule === undefined ? 'when you press Run' : routine.schedule.kind === 'files' ? 'when the next new file arrives' : 'when you press Run, since its one scheduled time has passed')
    : `at ${nextAfterKeep.toLocaleString(undefined, { weekday: 'short', hour: 'numeric', minute: '2-digit' })}, not straight away`
  /*
   * WAITING FOR YOUR OK (0.840). The routine was set to wait before this step, so the card is a question, not a
   * recovery: what the next step will do, the conversation to read first, and two answers. Stop here is Keep: this
   * run ends and the routine stays as it was.
   */
  if (asked) {
    const next = execution.step + 1
    const words = execution.steps[execution.step]
    return <div className="lc-recovery">
      <span className="lc-recovery__head">
        <strong className="lc-tone-amber">Waiting for your OK</strong>
        {` · Step ${String(next)} of ${String(execution.of)} · Run started ${new Date(execution.startedAt).toLocaleString()}`}
      </span>
      <span className="lc-recovery__line">{execution.reason ?? `Step ${String(next)} waits for your OK.`}</span>
      {words !== undefined && <span className="lc-recovery__line lc-recovery__next">
        {`Step ${String(next)}: ${words.length > 280 ? `${words.slice(0, 279).trimEnd()}…` : words}`}
      </span>}
      {execution.missionId !== undefined && onOpenMission !== undefined && <button type="button" className="lc-ghostbutton" title={execution.missionId}
        onClick={() => onOpenMission(execution.missionId!)}>Open the conversation</button>}
      <span className="lc-recovery__line">{`Stop here ends this run after step ${String(execution.step)}; the routine runs again from step 1 ${nextRun}.`}</span>
      <span className="lc-recovery__actions">
        <button type="button" className="lc-primarybutton" disabled={busy || recover === undefined}
          onClick={() => { void decide('continue') }}>{`Go ahead with step ${String(next)}`}</button>
        <button type="button" className="lc-ghostbutton" disabled={busy || recover === undefined}
          onClick={() => { void decide('keep') }}>Stop here</button>
      </span>
      {error !== undefined && <span role="alert" className="lc-recovery__line lc-tone-red">{error}</span>}
    </div>
  }
  // The head already says it waits for review; the host's reason says so again.
  const lastStep = execution.step >= execution.of
  const reason = (execution.reason ?? 'Dispatch outcome is uncertain. Nothing will be replayed automatically.')
    .replace(/^Review required: (\S)/, (_whole, first: string) => first.toUpperCase())
  return <div className="lc-recovery">
    <span className="lc-recovery__head">
      <strong className="lc-tone-amber">{execution.status === 'abandoned' ? 'Attempt abandoned' : 'Waiting for your review'}</strong>
      {` · Step ${String(execution.step)} of ${String(execution.of)} · Attempt started ${new Date(execution.startedAt).toLocaleString()}`}
    </span>
    <span className="lc-recovery__line">{reason}</span>
    {execution.missionId !== undefined && (onOpenMission !== undefined
      ? <button type="button" className="lc-ghostbutton" title={execution.missionId}
          onClick={() => onOpenMission(execution.missionId!)}>Open the saved conversation</button>
      : <span className="lc-recovery__line lc-mono">Saved mission: {execution.missionId}</span>)}
    {execution.status !== 'abandoned' && <>
      <details className="lc-recovery__details"><summary>Review saved steps and route</summary>
        <span className="lc-mono">{execution.route.runtime} · {execution.route.model} · {execution.route.mode}</span>
        <ol>{execution.steps.map((step, index) => <li key={index}>{step}</li>)}</ol>
      </details>
      {/*
        * WHY CONTINUE IS NOT THERE, said rather than left disabled.
        *
        * The 0.390 beta pass: step 2 of 2 interrupted, "Continue remaining
        * steps" greyed out with no reason -- there were no remaining steps --
        * and the only way out, Abandon, also removed the schedule. So the
        * last step offers no Continue, an unconfirmed earlier step says why
        * it cannot go on, and Keep puts the attempt down with the routine
        * left as it was.
        */}
      {/*
        * A hold the start itself answered (0.818, settledAtDispatch) is not an unconfirmed step: the 0.817 sweep's
        * routine read "Continue waits for step 1 to be confirmed finished. It was not" under "Step 2 is too long to
        * send", step 1 having answered. Its reason is the whole story, and a Continue would meet it again.
        */}
      {lastStep
        ? <span className="lc-recovery__line">This was the last step, so there is nothing left to continue.</span>
        : !execution.canContinue && <span className="lc-recovery__line">{execution.settledAtDispatch === true
          ? 'Continue would be refused the same way, so it is off until that changes.'
          : `Continue waits for step ${String(execution.step)} to be confirmed finished. It was not, so step ${String(execution.step + 1)} would build on work that may not be there.`}</span>}
      {/*
        * WHEN KEEP RUNS IT NEXT, said before the click (0.404). Keep starts
        * the routine's clock at the decision, so the next run is one
        * interval (or the next daily time) from now -- never at once, which
        * is what a routine held past its interval did until 0.404, shown
        * only after the click (the 0.402 beta retest).
        */}
      <span className="lc-recovery__line">{`Keep clears this attempt and leaves the routine as it was; its next run starts from step 1 ${nextRun}. Abandon also removes the schedule. Neither stops a runtime or undoes work.`}</span>
      <label className="lc-recovery__ack"><input type="checkbox" checked={reviewed} disabled={busy}
        onChange={(event) => setReviewed(event.target.checked)} /> I reviewed the saved conversation and external work, and whether the remaining work is still wanted.</label>
      <span className="lc-recovery__actions">
        {!lastStep && <button type="button" className="lc-ghostbutton" disabled={busy || !reviewed || !execution.canContinue || recover === undefined}
          onClick={() => { void decide('continue') }}>Continue remaining steps</button>}
        <button type="button" className="lc-ghostbutton" disabled={busy || !reviewed || recover === undefined}
          onClick={() => { void decide('keep') }}>{routine.schedule === undefined ? 'Keep the routine' : 'Keep the schedule'}</button>
        <button type="button" className="lc-ghostbutton" disabled={busy || !reviewed || recover === undefined}
          onClick={() => { void decide('abandon') }}>Abandon attempt and remove schedule</button>
      </span>
    </>}
    {error !== undefined && <span role="alert" className="lc-recovery__line lc-tone-red">{error}</span>}
  </div>
}
