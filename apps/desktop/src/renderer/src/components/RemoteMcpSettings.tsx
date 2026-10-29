import { useEffect, useState } from 'react'
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

export function RemoteMcpSettings({ api = typeof window === 'undefined' ? undefined : window.desktop?.remoteMcp, initial }: { readonly api?: RemoteMcpApi; readonly initial?: RemoteMcpState }) {
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
      <p>Locust must be open on this computer while an app uses it. The relay can read what an app asks and what Locust answers, as an app’s own servers can; it keeps none of it, and the keys are this computer’s.</p>
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
