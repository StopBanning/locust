import { useEffect, useRef, useState, type ReactElement, type ReactNode } from 'react'

import { spacedCode, type ChatAppApi, type ChatAppId, type ChatAppSettings } from '../../../shared/chat-apps.js'

/*
 * WHAT EACH CHAT APP'S ROW IN SETTINGS SHARES (0.832, ChatAppsSettings.tsx): its state, read and followed; the pairing
 * code or link kept fresh; its name and few words of state; and the code to send, with Copy beside it.
 */

/** One app's state in Settings: read, followed as it changes, and changed one press at a time. */
export function useChatApp(api: ChatAppApi | undefined, initial: ChatAppSettings | undefined) {
  const [state, setState] = useState<ChatAppSettings | undefined>(initial)
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState('')
  useEffect(() => {
    let alive = true
    void api?.settings().then((next) => { if (alive) setState(next) }, () => { if (alive) setFailure('This setting could not be read. It is unchanged.') })
    const stop = api?.onChanged((next) => { if (alive) setState(next) })
    return () => { alive = false; stop?.() }
  }, [api])
  const run = async (work: () => Promise<ChatAppSettings>): Promise<void> => {
    if (api === undefined || busy) return
    setBusy(true)
    setFailure('')
    try { setState(await work()) } catch { setFailure('This setting could not be changed. It is unchanged.') } finally { setBusy(false) }
  }
  return { state, busy, failure, run }
}

/** A pairing code or link runs out after ten minutes; while nothing is paired, a fresh one takes its place. */
export function useFreshPairing(api: ChatAppApi | undefined, state: ChatAppSettings | undefined, run: (work: () => Promise<ChatAppSettings>) => Promise<void>): void {
  const asked = useRef(false)
  const until = state?.code?.until ?? state?.link?.until
  const paired = state?.chat !== undefined
  useEffect(() => {
    if (api === undefined || state?.bot === undefined || paired) return
    if (until === undefined) {
      if (!asked.current) { asked.current = true; void run(() => api.newLink()) }
      return
    }
    const timer = setTimeout(() => { void run(() => api.newLink()) }, Math.max(1_000, Date.parse(until) - Date.now()))
    return () => clearTimeout(timer)
    // run is made fresh each render; the bot, the pairing and its end are what this follows.
  }, [api, state?.bot?.username, paired, until])
}

export const openLink = (url: string): void => { void window.desktop?.openLink(url).catch(() => undefined) }

/** The row's few words: off, connecting, waiting for the person to pair, or on. */
export function chatStatus(state: ChatAppSettings | undefined): string {
  if (state?.bot === undefined) return 'Off'
  if (state.chat === undefined) return state.connected ? 'Not paired yet' : 'Connecting…'
  return state.connected ? `On · ${state.chat.name}` : `On · ${state.chat.name} · not connected now`
}

export function ChatAppRow({ app, name, state, open, onToggle, failure, children }: {
  readonly app: ChatAppId
  readonly name: string
  readonly state: ChatAppSettings | undefined
  /** Off, the steps show only once Set up is pressed. */
  readonly open: boolean
  readonly onToggle: () => void
  readonly failure: string
  readonly children: ReactNode
}): ReactElement {
  const set = state?.bot !== undefined
  return (
    <li className="lc-chatapp" data-chat-app={app}>
      <div className="lc-chatapp__head">
        <span className="lc-chatapp__who">
          <span className="lc-chatapp__name">{name}</span>
          <span className="lc-chatapp__state">{chatStatus(state)}</span>
        </span>
        {!set && <button type="button" className="lc-button" aria-expanded={open} onClick={onToggle}>{open ? 'Hide' : 'Set up'}</button>}
      </div>
      {state?.message !== undefined && <p className="lc-settings__note" role="status">{state.message}</p>}
      {failure && <p className="lc-settings__note" role="status">{failure}</p>}
      {(set || open) && <div className="lc-chatapp__body">{children}</div>}
    </li>
  )
}

/**
 * TELL ME HERE WHEN SOMETHING NEEDS ME (0.837, shared/chat-apps.ts). Off until turned on. Says, before it is on, what
 * would leave this computer for the app's servers.
 */
export function ChatAlertsSwitch({ app, name, state, busy, onSet }: {
  readonly app: ChatAppId
  readonly name: string
  readonly state: ChatAppSettings
  readonly busy: boolean
  readonly onSet: (on: boolean) => void
}): ReactElement {
  const on = state.alerts === true
  return (
    <div className="lc-settingline lc-chatapp__alerts" data-chat-alerts={app}>
      <div className="lc-settingline__text">
        <h3 className="lc-settings__heading">Tell me here when something needs me</h3>
        <p className="lc-settings__lede">While you are away from this window, the cards that wait on you come here with Allow once and Deny, and so do Scout’s urgent finds and a teammate that hits its limit, by your phone’s rules: merged, a few a day, held in quiet hours. {name}’s servers can read who is asking, what the card asks to do and the first line of its file or command, with anything shaped like a key taken out.</p>
      </div>
      <button type="button" className={`lc-switch${on ? ' is-on' : ''}`} role="switch" aria-label={`Tell me in ${name} when something needs me`} aria-checked={on} disabled={busy} onClick={() => { onSet(!on) }}><span /></button>
    </div>
  )
}

/** The code that pairs an account, big enough to read out, with Copy beside it. */
export function PairingCode({ code, label }: { readonly code: string; readonly label: string }): ReactElement {
  const [copied, setCopied] = useState(false)
  return (
    <span className="lc-chatapp__code">
      <code className="lc-chatapp__digits" aria-label={label}>{spacedCode(code)}</code>
      <button type="button" className="lc-control lc-control--boxed" onClick={() => {
        void navigator.clipboard?.writeText(code).then(() => { setCopied(true); setTimeout(() => setCopied(false), 2_500) }, () => undefined)
      }}>{copied ? 'Copied' : 'Copy the code'}</button>
    </span>
  )
}
