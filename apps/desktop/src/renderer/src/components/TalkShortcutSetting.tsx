import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'

import { DEFAULT_TALK_SHORTCUT, shortcutFromPress, TALK_SHORTCUT_SET, type TalkShortcutApi, type TalkShortcutState } from '../../../shared/talk-shortcut.js'
import { failureSaid } from '../onTheComputer.js'
import { OnTheComputer } from './OnTheComputer.js'

/**
 * TALK FROM ANY APP (0.755, shared/talk-shortcut.ts): the keys, on or off, and changed by pressing the new ones. It
 * says plainly when Windows would not give the keys to Locust -- another app holds them -- or when they are waiting
 * for Talk to be ready, rather than a switch that looks on and does nothing.
 */
export function TalkShortcutSetting({ heading, api = typeof window === 'undefined' ? undefined : window.desktop?.talkShortcut, platform = typeof window === 'undefined' ? undefined : window.desktop?.platform }: {
  /** Settings' own heading, so its search finds this line (settings-pages-are-real). */
  readonly heading: ReactNode
  readonly api?: TalkShortcutApi
  readonly platform?: string
}) {
  const [state, setState] = useState<TalkShortcutState>()
  const [recording, setRecording] = useState(false)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  useEffect(() => {
    let alive = true
    void api?.read().then((read) => { if (alive) setState(read) }).catch(() => { if (alive) setMessage('The shortcut could not be read. Your keys are as they were; open Settings again to see them.') })
    return () => { alive = false }
  }, [api])
  if (api === undefined || platform !== 'win32') return null
  const keys = state?.keys
  const change = async (next: string | false): Promise<void> => {
    setBusy(true); setMessage('')
    try {
      setState(await api.set(next))
    } catch (error) {
      setMessage(failureSaid(error, 'The shortcut was not changed. Try again.'))
    } finally { setBusy(false) }
  }
  return <section className="lc-settings__section lc-settings__section--line" data-setting="talk-shortcut"><OnTheComputer channel={TALK_SHORTCUT_SET}><div className="lc-settingline">
    <div className="lc-settingline__text">
      {heading}
      <p className="lc-settings__lede">{keys === undefined
        ? 'Off. Turn it on to start Talk from any app with a key press.'
        : `Press ${keys} anywhere to bring Locust up and start Talk; press it again to end the call. In a teammate’s conversation the call is with them.`}</p>
      {state?.why !== undefined && <p className="lc-settings__note" role="status">{state.why}</p>}
      {recording && <p className="lc-settings__note" role="status">Press the new keys: Ctrl or Alt, then a letter, number, F-key or Space. Esc keeps the ones you have.</p>}
      {message && <p className="lc-settings__note" role="status">{message}</p>}
    </div>
    <span className="lc-talkkeys">
      {keys !== undefined && <button
        type="button"
        className={`lc-control lc-control--boxed lc-talkkeys__keys${recording ? ' is-recording' : ''}`}
        aria-label={recording ? 'Press the new keys for Talk' : `Change Talk’s keys, now ${keys}`}
        disabled={busy}
        onClick={() => { setRecording(!recording) }}
        onBlur={() => { setRecording(false) }}
        onKeyDown={(event) => {
          if (!recording) return
          event.preventDefault()
          if (event.key === 'Escape') { setRecording(false); return }
          const next = shortcutFromPress(event)
          if (next === undefined) return
          setRecording(false)
          void change(next)
        }}
      >{recording ? 'Press keys…' : keys}</button>}
      <button type="button" className={`lc-switch${keys !== undefined ? ' is-on' : ''}`} role="switch" aria-label="Talk from any app" aria-checked={keys !== undefined} disabled={state === undefined || busy}
        onClick={() => { setRecording(false); void change(keys === undefined ? DEFAULT_TALK_SHORTCUT : false) }}><span /></button>
    </span>
  </div></OnTheComputer></section>
}
