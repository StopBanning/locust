import { useEffect, useRef, useState, type ReactElement, type ReactNode } from 'react'

import { policyFeatureRefusal } from '../../../shared/organization-policy.js'
import type { TelegramApi, TelegramSettings as TelegramState } from '../../../shared/telegram.js'
import { ManagedPolicyNote, useOrganizationPolicy } from '../organizationPolicy.js'
import { QrCode } from './PhoneSettings.js'

export const BOTFATHER_LINK = 'https://t.me/BotFather'

/*
 * YOUR TEAMMATES IN TELEGRAM (0.831, shared/telegram.ts). Three steps, each said where it is done: make a bot with
 * @BotFather, paste its token here, and press Start in the chat the link opens. The token goes in a masked field and
 * comes back from Locust never again -- only the bot's name does.
 */
export function TelegramSettings({ heading, more, api = typeof window === 'undefined' ? undefined : window.desktop?.telegram, initial }: {
  readonly heading?: ReactNode
  readonly more?: ReactNode
  readonly api?: TelegramApi
  /** What to show before Locust has answered: a test draws each state from it. */
  readonly initial?: TelegramState
}): ReactElement {
  const [state, setState] = useState<TelegramState | undefined>(initial)
  const [token, setToken] = useState('')
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState('')
  const [copied, setCopied] = useState(false)
  const managed = policyFeatureRefusal(useOrganizationPolicy(), 'chatApps') !== undefined
  const asked = useRef(false)

  useEffect(() => {
    let alive = true
    void api?.settings().then((next) => { if (alive) setState(next) }, () => { if (alive) setFailure('This setting could not be read. It is unchanged.') })
    const stop = api?.onChanged((next) => { if (alive) setState(next) })
    return () => { alive = false; stop?.() }
  }, [api])

  const run = async (work: () => Promise<TelegramState>): Promise<void> => {
    if (!api || busy) return
    setBusy(true); setFailure('')
    try { setState(await work()) } catch { setFailure('This setting could not be changed. It is unchanged.') } finally { setBusy(false) }
  }
  const open = (url: string): void => { void window.desktop?.openLink(url).catch(() => undefined) }

  // A link runs out after ten minutes; while no chat is paired, a fresh one takes its place.
  const link = state?.link
  const paired = state?.chat !== undefined
  useEffect(() => {
    if (managed || state?.bot === undefined || paired) return
    if (link === undefined) {
      if (!asked.current) { asked.current = true; void run(() => api!.newLink()) }
      return
    }
    const timer = setTimeout(() => { void run(() => api!.newLink()) }, Math.max(1_000, Date.parse(link.until) - Date.now()))
    return () => clearTimeout(timer)
    // run is made fresh each render; the bot, the chat and the link are what this follows.
  }, [managed, state?.bot?.username, paired, link?.until])

  const bot = state?.bot
  const lede = managed ? undefined
    : bot === undefined
      ? 'Off. Write to your teammates from Telegram, get their answers there, and answer their approval cards, while Locust is open on this computer.'
      : !paired
        ? `Connected to @${bot.username}. Now pair your own chat with it: open the link, then press Start in Telegram.`
        : `On. In Telegram, write /name and a message to @${bot.username}: it reaches that teammate, and the answer comes back there.`

  return (
    <>
      <div className="lc-settingline">
        <div className="lc-settingline__text">
          {heading}
          {managed && <ManagedPolicyNote />}
          {lede !== undefined && <p className="lc-settings__lede">{lede}</p>}
          {state?.message !== undefined && <p className="lc-settings__note" role="status">{state.message}</p>}
          {failure && <p className="lc-settings__note" role="status">{failure}</p>}
        </div>
      </div>

      {!managed && bot === undefined && (
        <div className="lc-telegram__steps" data-telegram="token">
          <p className="lc-settings__note">1. In Telegram, open @BotFather and send /newbot. Give your bot any name; BotFather answers with its token.</p>
          <span><button type="button" className="lc-control lc-control--boxed" onClick={() => open(BOTFATHER_LINK)}>Open BotFather</button></span>
          <p className="lc-settings__note">2. Paste the token here. It is kept encrypted on this computer and sent only to Telegram.</p>
          <form className="lc-telegram__token" onSubmit={(event) => {
            event.preventDefault()
            const given = token
            void run(() => api!.setToken(given)).then(() => setToken(''))
          }}>
            <input className="lc-input" type="password" aria-label="Your bot’s token from BotFather" autoComplete="off" spellCheck={false} value={token} disabled={busy}
              onChange={(event) => { setToken(event.target.value) }} placeholder="123456789:AAH…" />
            <button type="submit" className="lc-button" disabled={busy || token.trim().length === 0}>Connect</button>
          </form>
        </div>
      )}

      {!managed && bot !== undefined && !paired && (
        <div className="lc-phone" data-telegram="pair">
          {link !== undefined && <QrCode text={link.url} label="A code for your phone’s camera. It opens your bot in Telegram to pair your chat." />}
          <div className="lc-phone__side">
            <p className="lc-settings__note" role="status">{state?.connected === true ? `Reading @${bot.username}’s messages.` : 'Connecting to Telegram…'}</p>
            {link !== undefined
              ? <>
                <p className="lc-settings__note">3. Scan the code with your phone, or open the link here, and press Start. It pairs one chat, once, within ten minutes.</p>
                <span className="lc-telegram__buttons">
                  <button type="button" className="lc-control lc-control--boxed" onClick={() => open(link.url)}>Open in Telegram</button>
                  <button type="button" className="lc-control lc-control--boxed" onClick={() => {
                    void navigator.clipboard?.writeText(link.url).then(() => { setCopied(true); setTimeout(() => setCopied(false), 2_500) }, () => undefined)
                  }}>{copied ? 'Copied' : 'Copy the link'}</button>
                </span>
              </>
              : <span><button type="button" className="lc-control lc-control--boxed" disabled={busy} onClick={() => { void run(() => api!.newLink()) }}>Make a pairing link</button></span>}
            <span><button type="button" className="lc-button" disabled={busy} onClick={() => { void run(() => api!.remove()) }}>Remove the bot</button></span>
          </div>
        </div>
      )}

      {!managed && bot !== undefined && paired && state?.chat !== undefined && (
        <>
          <ul className="lc-phone__devices" aria-label="Your Telegram chat">
            <li className="lc-phone__device">
              <span className="lc-phone__who">
                <span className="lc-phone__name">{state.chat.name}{state.chat.username === undefined ? '' : ` (@${state.chat.username})`}</span>
                <span className="lc-phone__state">{state.connected ? `Paired with @${bot.username}` : `Paired with @${bot.username} · not connected now`}</span>
              </span>
              <button type="button" className="lc-button" disabled={busy} onClick={() => { asked.current = true; void run(() => api!.newLink()) }}>Pair another chat</button>
              <button type="button" className="lc-button" disabled={busy} onClick={() => { void run(() => api!.remove()) }}>Remove</button>
            </li>
          </ul>
          {link !== undefined && (
            <div className="lc-phone" data-telegram="pair-again">
              <QrCode text={link.url} label="A code for your phone’s camera. It pairs another Telegram chat in place of this one." />
              <div className="lc-phone__side">
                <p className="lc-settings__note">The chat that presses Start with this link takes the place of {state.chat.name}’s, within ten minutes.</p>
                <span><button type="button" className="lc-control lc-control--boxed" onClick={() => open(link.url)}>Open in Telegram</button></span>
              </div>
            </div>
          )}
          <p className="lc-settings__note">Telegram’s servers can read what you and your teammates write there: a bot’s chat is not end-to-end encrypted. Locust takes out anything shaped like a key before it sends, but keep secrets out of it. Approval cards there offer Allow once and Deny; Always stays here.</p>
        </>
      )}
      {more}
    </>
  )
}
