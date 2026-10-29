import { useEffect, useState } from 'react'
import type { RemoteMcpApi, RemoteMcpAsking } from '../../../shared/remote-mcp.js'

/**
 * AN APP ASKS TO USE THE TEAM FROM ANYWHERE (0.841, main/remote-mcp.ts). The only place a sign-in is answered: never
 * the page the app sent the person to, never the relay. The card names the app, where it sends the person back, and
 * the six digits that page shows, so the person knows it is the sign-in they started.
 */
export function RemoteConnectCard({ api = typeof window === 'undefined' ? undefined : window.desktop?.remoteMcp, initial }: { readonly api?: RemoteMcpApi; readonly initial?: readonly RemoteMcpAsking[] }) {
  const [asking, setAsking] = useState<readonly RemoteMcpAsking[]>(initial ?? [])
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    if (api === undefined) return undefined
    let alive = true
    void api.settings().then((state) => { if (alive) setAsking(state.asking) }, () => undefined)
    const stop = api.onChanged((state) => { if (alive) setAsking(state.asking) })
    return () => { alive = false; stop() }
  }, [api])
  const one = asking[0]
  if (one === undefined) return null
  const decide = async (allow: boolean): Promise<void> => {
    if (busy || api === undefined) return
    setBusy(true)
    try { setAsking((await api.decide(one.request, allow)).asking) } catch { /* The card stays; it can be answered again. */ } finally { setBusy(false) }
  }
  return (
    <section className="lc-remote-connect" role="alertdialog" aria-labelledby="lc-remote-connect-title" aria-describedby="lc-remote-connect-said">
      <div className="lc-remote-connect__text">
        <h2 id="lc-remote-connect-title" className="lc-remote-connect__title">{one.name} wants to use your Locust team</h2>
        <p id="lc-remote-connect-said" className="lc-remote-connect__said">
          From anywhere, through Locust’s relay. It sends you back to {one.host}. Allow it only if you started this, and its sign-in page shows <b className="lc-remote-connect__match">{one.match.slice(0, 3)} {one.match.slice(3)}</b>.
        </p>
        <p className="lc-remote-connect__note">It can then ask your teammates and run your routines by the rules under Let your other AI apps use Locust. Remove it there at any time.</p>
      </div>
      <div className="lc-remote-connect__acts">
        <button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { void decide(true) }}>Allow</button>
        <button type="button" className="lc-control" disabled={busy} onClick={() => { void decide(false) }}>Deny</button>
      </div>
    </section>
  )
}
