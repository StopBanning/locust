import { useState, type ReactElement } from 'react'
import type { PublicCompare } from '../../../shared/compare.js'
import type { PublicTeammate } from '../../../shared/ipc.js'
import { keptInstructions } from '../../../shared/tune.js'

/** KEEP IS NOT SAVE (0.808): the person's second, explicit choice changes future turns only. */
export function KeptInstructions({ compare, teammate, onUse }: {
  readonly compare: PublicCompare
  readonly teammate: PublicTeammate | undefined
  readonly onUse: (teammate: PublicTeammate, instructions: string) => Promise<string | undefined>
}): ReactElement | null {
  const [dismissed, setDismissed] = useState(false)
  const [applied, setApplied] = useState(false)
  const [busy, setBusy] = useState(false)
  const [problem, setProblem] = useState<string>()
  const instructions = keptInstructions(compare, teammate)
  if (applied) return <div className="lc-tuneoffer" role="status">These instructions take effect from the next turn. You can edit them in Edit teammate.</div>
  if (dismissed || teammate === undefined || instructions === undefined) return null
  return <div className="lc-tuneoffer">
    <span>The kept column used different standing instructions.</span>
    <details><summary>Review instructions</summary><pre>{instructions.length === 0 ? 'No standing instructions.' : instructions}</pre></details>
    <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => {
      if (busy) return
      setBusy(true)
      void onUse(teammate, instructions).then((why) => {
        if (why === undefined) setApplied(true)
        else setProblem(why)
      }).catch(() => setProblem('The instructions could not be saved. The teammate is unchanged.')).finally(() => setBusy(false))
    }}>Use these instructions for {teammate.name}</button>
    <button type="button" className="lc-ghostbutton" disabled={busy} onClick={() => setDismissed(true)}>Not now</button>
    {problem !== undefined && <span className="lc-tone-red" role="alert">{problem}</span>}
  </div>
}
