import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'

import type { TurnUndoState } from '../../../shared/ipc.js'

/**
 * UNDO A TURN (0.674): the way back from what one finished turn changed in the folder.
 *
 * Claude Code rewinds; a Locust turn wrote straight into the folder and there was no way back short of
 * the person's own git. Locust now keeps the folder as it was before each turn that may write
 * (main/checkpoints.ts), and the turn's foot offers to put back what the turn changed. Asked first, in
 * words, because it changes files: how many, and that a file changed since is left as it is. Afterwards
 * it says what it did, and names each file it left alone and why.
 *
 * Nothing is drawn for a turn with no way back -- one that changed nothing, one that shared its folder
 * with another run (what changed cannot be told apart), one from before 0.674 -- so a turn reads as it
 * always has unless there is something to offer.
 */
export function TurnUndo({ runId, onUndone }: {
  readonly runId: string
  /** Told once the turn's files are put back, so the card above them can say so (0.840). */
  readonly onUndone?: () => void
}): ReactElement | null {
  const [state, setState] = useState<TurnUndoState | undefined>(undefined)
  const putBack = state?.kind === 'undone' && state.restored.length > 0
  useEffect(() => {
    if (putBack) onUndone?.()
  }, [putBack, onUndone])
  const [asking, setAsking] = useState(false)
  const [working, setWorking] = useState(false)
  useEffect(() => {
    let live = true
    let timer: ReturnType<typeof setTimeout> | undefined
    const bridge = window.desktop
    if (bridge?.turnUndoStates === undefined) return
    /*
     * The turn reads as over a moment before its record is written: the folder is looked at again after
     * the run's last event (drive-a-turn-can-be-undone: the foot asked once, heard nothing, and offered
     * nothing until Locust was opened again). So a turn with no record yet is asked again, for a while.
     */
    let tries = 0
    const ask = (): void => {
      void bridge.turnUndoStates([runId]).then((states) => {
        if (!live) return
        const now = states[runId]
        setState(now)
        tries += 1
        if ((now === undefined || now.kind === 'none') && tries < 20) timer = setTimeout(ask, 750)
      }).catch(() => undefined)
    }
    ask()
    return () => {
      live = false
      if (timer !== undefined) clearTimeout(timer)
    }
  }, [runId])
  if (state === undefined || state.kind === 'none' || state.kind === 'shared') return null
  if (state.kind === 'undone') {
    const put = state.restored.length
    return (
      <div className="lc-turnundo is-done" role="status">
        <span>{put === 0 ? 'Nothing was put back.' : `Undone: ${put === 1 ? '1 file put back as it was' : `${String(put)} files put back as they were`} before this turn.`}</span>
        {state.leftAlone.length > 0 && (
          <ul className="lc-turnundo__left">
            {state.leftAlone.map((file) => (
              <li key={file.path}>
                <span className="lc-mono">{file.path}</span> left as it is: {file.why}
              </li>
            ))}
          </ul>
        )}
      </div>
    )
  }
  const files = state.files === 1 ? '1 file' : `${String(state.files)} files`
  if (!asking) {
    return (
      <div className="lc-turnundo">
        <button type="button" className="lc-turnundo__ask" onClick={() => setAsking(true)}>
          Undo these changes
        </button>
      </div>
    )
  }
  return (
    <div className="lc-turnundo is-asking" role="group" aria-label="Undo this turn's changes">
      <span>
        Put back the {files} this turn changed, as they were before it? A file changed since is left as it is.
        {state.notKept.length > 0 && ` Too large to have been kept: ${state.notKept.join(', ')}.`}
      </span>
      <span className="lc-turnundo__actions">
        <button
          type="button"
          className="lc-turnundo__go"
          disabled={working}
          onClick={() => {
            const bridge = window.desktop
            if (bridge?.undoTurn === undefined) return
            setWorking(true)
            void bridge.undoTurn(runId).then((next) => {
              setState(next)
              setAsking(false)
            }).finally(() => setWorking(false))
          }}
        >
          {working ? 'Undoing…' : 'Undo'}
        </button>
        <button type="button" className="lc-turnundo__cancel" disabled={working} onClick={() => setAsking(false)}>
          Keep them
        </button>
      </span>
    </div>
  )
}
