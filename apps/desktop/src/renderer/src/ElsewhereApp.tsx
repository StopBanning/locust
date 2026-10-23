import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'

/**
 * THIS COMPUTER'S WINDOW WHILE A BROWSER HAS IT (0.830, main/desk-host.ts). Loaded at ?elsewhere=1 in place of the app,
 * so nothing of the app runs here -- no queued message goes twice, no setting is saved over the browser's -- while
 * the browser is the window. Teammates keep working on this computer all the same; Use Locust here takes it back,
 * and the browser says so.
 */
export function ElsewhereApp(): ReactElement {
  const [device, setDevice] = useState<string>()
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    void window.desktop?.deskWhere?.().then((where) => { setDevice(where.device) }, () => undefined)
  }, [])
  return (
    <main className="lc-elsewhere" aria-label="Locust is open in a browser">
      <div className="lc-elsewhere__box">
        <h1 className="lc-elsewhere__title">Locust is open in a browser</h1>
        <p className="lc-elsewhere__said">{device === undefined
          ? 'Your teammates keep working on this computer, and the browser shows what they do.'
          : `${device} has it now. Your teammates keep working on this computer, and the browser shows what they do.`}</p>
        <button type="button" className="lc-control lc-control--boxed lc-elsewhere__back" disabled={busy}
          onClick={() => { setBusy(true); void window.desktop?.deskTakeBack?.().finally(() => setBusy(false)) }}>
          Use Locust here
        </button>
      </div>
    </main>
  )
}
