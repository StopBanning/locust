import { useEffect, useState } from 'react'
import type { LoginItemState } from '../../../shared/ipc.js'
import type { RemoteMcpApi, RemoteMcpApp, RemoteMcpState } from '../../../shared/remote-mcp.js'
import { CopyButton } from './CopyButton.js'

/**
 * USE FROM ANYWHERE (0.841, main/remote-mcp.ts). Claude on the web, Claude Duet, and other apps that take a remote MCP
 * server, reach the team here at one address on Locust's relay. Each app connects once, on a card in this window,
 * and is listed here with Remove.
 */
const when = (iso: string): string => {
  const at = new Date(iso)
  return Number.isNaN(at.getTime()) ? '' : at.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
}
export const appLine = (app: RemoteMcpApp): string =>
  `${app.host} · connected ${when(app.added)}${app.used === undefined ? '' : ` · last used ${when(app.used)}`}`

/**
 * KEEP IT RUNNING (0.842, Colin: "right now it only works if it's open on my pc"). Nothing on the computer hears the
 * relay once Locust has quit, so no app can open it. Kept in the tray when its window closes, and started when the
 * person signs in to Windows, it is reachable whenever the computer is on and awake. The Startup switches, here too.
 */
export interface StayingReachable {
  readonly loginItem: LoginItemState
  readonly keepRunning: boolean
  readonly onLoginItemChange?: (openAtLogin: boolean) => void
  readonly onKeepRunningChange?: (keepRunning: boolean) => void
}
/** The button's one act: each Startup switch that is off and can be on, turned on; nothing turned off. */
export function keepItRunning(stay: StayingReachable): void {
  if (!stay.keepRunning) stay.onKeepRunningChange?.(true)
  if (stay.loginItem.available === true && !stay.loginItem.openAtLogin) stay.onLoginItemChange?.(true)
}
export const reachable = (stay: StayingReachable): boolean => stay.keepRunning && (stay.loginItem.openAtLogin || stay.loginItem.available !== true)

export function RemoteMcpSettings({ api = typeof window === 'undefined' ? undefined : window.desktop?.remoteMcp, initial, stay }: { readonly api?: RemoteMcpApi; readonly initial?: RemoteMcpState; readonly stay?: StayingReachable }) {
  const [state, setState] = useState<RemoteMcpState | undefined>(initial)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    if (api === undefined) return undefined
    let alive = true
    void api.settings().then((value) => { if (alive) setState(value) }, () => undefined)
    const stop = api.onChanged((value) => { if (alive) setState(value) })
    return () => { alive = false; stop() }
  }, [api])
  if (api === undefined && initial === undefined) return null
  const run = async (work: () => Promise<RemoteMcpState>): Promise<void> => {
    if (busy) return
    setBusy(true)
    try { setState(await work()) } catch { /* The state shown stays as it was. */ } finally { setBusy(false) }
  }
  const on = state?.enabled === true
  return <>
    <div className="lc-settingline lc-settingline--sub">
      <div className="lc-settingline__text">
        <h3 className="lc-settings__heading">Use from anywhere</h3>
        <p className="lc-settings__lede">{on
          ? 'On. Claude on the web, Claude Duet and other apps that take a remote MCP server can use your team through Locust’s relay, by the same rules as an app on this computer. Each one connects only after you allow it on a card here.'
          : 'Off. Turn on to use your team from Claude on the web, Claude Duet, or another app that takes a remote MCP server. It goes through the same relay as your phone from anywhere.'}</p>
      </div>
      <button type="button" className={`lc-switch${on ? ' is-on' : ''}`} role="switch" aria-label="Use your team from anywhere" aria-checked={on} disabled={busy || state === undefined}
        onClick={() => { void run(() => api!.setEnabled(!on)) }}><span /></button>
    </div>
    {on && state?.address !== undefined && <div className="lc-settings__note lc-remote-mcp">
      <p>In Claude, open Settings, then Connectors, choose Add custom connector, and paste this address:</p>
      <div className="lc-mcp-setupwrap"><pre className="lc-mcp-setup lc-mono">{state.address}</pre>
        <CopyButton className="lc-mcp-setup__copy" label="the address" text={state.address} /></div>
      {stay === undefined
        ? <p>Locust must be open on this computer while an app uses it.</p>
        : reachable(stay)
          ? <p>Locust stays reachable while this computer is on and awake: {stay.loginItem.openAtLogin ? 'it starts when you sign in to Windows, and ' : ''}it keeps running in the tray when you close its window. Quit from the tray, and no app can open it again.</p>
          : <div className="lc-remote-mcp__stay">
            <p>An app can reach Locust only while it is running here: once it quits, nothing on this computer can open it. Keep it running in the tray when you close its window{stay.loginItem.available === true ? ', and start it when you sign in to Windows' : ''}.</p>
            <button type="button" className="lc-button" onClick={() => keepItRunning(stay)}>Keep it running</button>
          </div>}
      <p>The relay can read what an app asks and what Locust answers, as an app’s own servers can; it keeps none of it, and the keys are this computer’s.</p>
    </div>}
    {on && state !== undefined && state.apps.length > 0 && <ul className="lc-phone__devices" aria-label="Apps that can use your team from anywhere">
      {state.apps.map((app) => (
        <li key={app.id} className="lc-phone__device">
          <span className="lc-phone__who">
            <span className="lc-phone__name">{app.name}</span>
            <span className="lc-phone__state">{appLine(app)}</span>
          </span>
          <button type="button" className="lc-button" disabled={busy} onClick={() => { void run(() => api!.remove(app.id)) }}>Remove</button>
        </li>
      ))}
    </ul>}
    {state?.message !== undefined && <p className="lc-settings__note" role="status">{state.message}</p>}
  </>
}
