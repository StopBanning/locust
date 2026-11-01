import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'

import type { PublicConnector } from '../../../shared/ipc.js'
import { connectorDetail, connectorReport, connectorStateWords, connectorWhere } from '../connectorHealth.js'

/**
 * THE CONNECTORS THIS MACHINE HAS, AND HOW EACH IS (W8, 0.567), in Settings >
 * Connectors: read from the host's cached `claude mcp list` reading, its
 * state in words and what to do, when it last worked, and Copy report for
 * someone helping. Check again asks the host again; it re-reads when its
 * reading is over five minutes old (a read takes as long as the slowest
 * connector, up to 20 seconds).
 */
export function ConnectorHealth({ now = () => new Date() }: { readonly now?: () => Date }): ReactElement {
  const [connectors, setConnectors] = useState<readonly PublicConnector[]>()
  // Reading from the first frame (0.849): starting false drew "could not be read" before the first read began.
  const [reading, setReading] = useState(() => typeof window !== 'undefined' && window.desktop !== undefined)
  const [notice, setNotice] = useState<string>()

  const read = (): void => {
    const bridge = window.desktop
    if (bridge === undefined) return
    setReading(true)
    void bridge
      .listConnectors()
      .then((answer) => {
        if (answer.ok) setConnectors(answer.data.connectors)
        else setNotice(answer.error.message)
      })
      .catch(() => setNotice('The connectors could not be read. Check again in a moment.'))
      .finally(() => setReading(false))
  }
  useEffect(read, [])

  const copy = (): void => {
    const text = connectorReport(connectors ?? [], now())
    setNotice(undefined)
    void navigator.clipboard
      .writeText(text)
      .then(() => setNotice('Report copied: names, states, timings and hosts only.'))
      .catch(() => setNotice('The report could not be copied; your connectors are as listed above. Press Copy report again.'))
  }

  return (
    <div className="lc-connectorhealth" aria-label="Connectors on this machine">
      {connectors === undefined ? (
        <p className="lc-settings__note">{reading ? 'Reading your connectors…' : 'Your connectors could not be read just now; your teammates keep the ones that last worked. Check again in a moment.'}</p>
      ) : connectors.length === 0 ? (
        <p className="lc-settings__note">Your Claude Code has no connectors.</p>
      ) : (
        <ul className="lc-connectorhealth__list">
          {connectors.map((connector) => {
            const words = connectorStateWords(connector)
            return (
              <li key={connector.name} className="lc-connectorhealth__row" data-status={connector.status}>
                <span className={`lc-connectorhealth__state lc-connectorhealth__state--${words.tone}`}>{words.state}</span>
                <span className="lc-connectorhealth__name">{connector.name}</span>
                <span className="lc-connectorhealth__where lc-mono">{connectorWhere(connector.location)}</span>
                {connectorDetail(connector, now()) !== undefined && <span className="lc-connectorhealth__detail">{connectorDetail(connector, now())}</span>}
              </li>
            )
          })}
        </ul>
      )}
      <div className="lc-connectorhealth__actions">
        <button type="button" className="lc-button" disabled={reading} onClick={read}>
          {reading ? 'Checking…' : 'Check again'}
        </button>
        <button type="button" className="lc-button" disabled={connectors === undefined} onClick={copy}>
          Copy report
        </button>
      </div>
      {notice !== undefined && <p className="lc-settings__note">{notice}</p>}
    </div>
  )
}
