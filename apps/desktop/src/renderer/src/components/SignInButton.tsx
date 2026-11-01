import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'

import { signInCommand, signInOpenedLine } from '../../../shared/runtime-install.js'
import { runtimeDisplayName } from '../../../shared/runtimes.js'
import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import { SIGN_IN_OPENED_EVENT } from '../signInEvents.js'
import { onTheComputer } from '../onTheComputer.js'
import { RUNTIME_SIGN_IN_CHANNEL } from '../../../shared/ipc.js'

/** Runtimes whose sign-in command removes the current sign-in before it finishes (measured). */
const SIGNS_OUT_AT_START: ReadonlySet<string> = new Set(['codex'])

/**
 * SIGN IN, as a button.
 *
 * The row used to print `run muse login` and stop. Colin, 2026-09-22: signed
 * in to Meta in his browser, told to sign in again, with no idea where. A CLI
 * cannot borrow the browser's session, so the sign-in itself stays; the
 * terminal hunt goes. Pressing this opens the runtime's own sign-in in a
 * window of its own, and the row turns ready by itself once Locust is back
 * in front and asks again.
 *
 * The command stays reachable: it is the button's title, and it is what the
 * row falls back to when the window cannot be opened.
 */
export function SignInButton({ runtime, again = false }: { readonly runtime: string; readonly again?: boolean }): ReactElement | null {
  const [state, setState] = useState<'idle' | 'confirm' | 'opening' | 'opened' | { readonly failed: string }>('idle')
  const command = signInCommand(runtime, again)
  // Back in front means the window was dealt with, one way or the other. If
  // it signed in, discovery's own focus re-probe turns the row READY and this
  // button goes with it; if it was closed unfinished, the row must offer the
  // button again rather than keep pointing at a window that is gone.
  //
  // And when the window itself closes (0.545): Sol, on 0.544, closed Codex's
  // login unanswered and the row still said "Finish in the window that
  // opened" twenty seconds later, pointing at a window that was gone.
  useEffect(() => {
    if (state !== 'opened') return
    const back = (): void => setState('idle')
    const armed = window.setTimeout(() => window.addEventListener('focus', back), 1000)
    const stopClosed = window.desktop?.onSignInClosed?.((closed) => {
      if (closed === runtime) back()
    })
    return () => {
      window.clearTimeout(armed)
      window.removeEventListener('focus', back)
      stopClosed?.()
    }
  }, [state, runtime])
  if (command === undefined) return null
  const stays = onTheComputer(RUNTIME_SIGN_IN_CHANNEL)
  if (state === 'opened') {
    return <span className="lc-runtimecell__signin">{signInOpenedLine(runtime, again)}</span>
  }
  if (typeof state === 'object') {
    return (
      <span className="lc-runtimecell__signin" title={state.failed}>
        <span className="lc-mono">{command}</span>
      </span>
    )
  }
  const open = (): void => {
    const bridge = window.desktop
    if (!bridge) return
    setState('opening')
    void bridge
      .signInRuntime(runtime, again)
      .then((response) => {
        setState(response.ok ? 'opened' : { failed: `${response.what} ${response.next}` })
        // The window asks again the moment the person comes back to it.
        if (response.ok) window.dispatchEvent(new Event(SIGN_IN_OPENED_EVENT))
      })
      .catch(() => setState({ failed: 'The sign-in window could not be opened. Run the command shown in a terminal.' }))
  }
  /*
   * SIGNING IN AGAIN CAN SIGN YOU OUT (0.545). Sol, on 0.544, opened Codex's
   * login and closed it unanswered; Codex was signed out from then on. So
   * before the window opens, the row says so, and the person chooses.
   *
   * Measured 10/02 on a throwaway CODEX_HOME: `codex login` deletes the
   * sign-in the moment it starts, before the browser has been answered. So
   * for Codex the row says it plainly, not "may".
   */
  if (state === 'confirm') {
    const name = runtimeDisplayName(runtime as MissionRuntimeId)
    const warning = SIGNS_OUT_AT_START.has(runtime)
      ? `${name} signs out as soon as this opens, and stays signed out until you finish there.`
      : `Until you finish signing in there, ${name} may stay signed out.`
    return (
      <span className="lc-runtimecell__confirm" role="group" aria-label={`Sign in to ${name} again`}>
        <span className="lc-runtimecell__signin">{warning}</span>
        <button type="button" className="lc-runtimecell__again" onClick={open}>
          Open it
        </button>
        <button type="button" className="lc-runtimecell__again" onClick={() => setState('idle')}>
          Cancel
        </button>
      </span>
    )
  }
  return (
    <button
      type="button"
      className={again ? 'lc-runtimecell__again' : 'lc-runtimecell__install'}
      // The sign-in window opens on the computer, so a browser's button says so on it (0.849).
      title={stays ?? (again ? `Opens a window that runs: ${command.replace(/^run /, '')}. Sign in there as this account or another; the agent uses whichever signed in last.` : `Opens a window that runs: ${command.replace(/^run /, '')}`)}
      disabled={state === 'opening' || stays !== undefined}
      onClick={again ? () => setState('confirm') : open}
    >
      {again ? 'Sign in again' : 'Sign in'}
    </button>
  )
}
