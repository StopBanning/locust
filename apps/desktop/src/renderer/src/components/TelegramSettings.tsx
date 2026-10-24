import { useState, type ReactElement } from 'react'

import type { ChatAppApi, ChatAppSettings } from '../../../shared/chat-apps.js'
import { ChatAppRow, openLink, useChatApp, useFreshPairing } from './ChatAppRow.js'
import { QrCode } from './PhoneSettings.js'

export const BOTFATHER_LINK = 'https://t.me/BotFather'

/*
 * YOUR TEAMMATES IN TELEGRAM (0.831, shared/telegram.ts; a row of Chat apps since 0.832). Three steps, each said where
 * it is done: make a bot with @BotFather, paste its token here, and press Start in the chat the link opens. The token
 * goes in a masked field and comes back from Locust never again -- only the bot's name does.
 */
export function TelegramSettings({ api = typeof window === 'undefined' ? undefined : window.desktop?.telegram, initial, open: opened = false }: {
  readonly api?: ChatAppApi
  /** What to show before Locust has answered: a test draws each state from it. */
  readonly initial?: ChatAppSettings
  readonly open?: boolean
}): ReactElement {
  const { state, busy, failure, run } = useChatApp(api, initial)
  const [open, setOpen] = useState(opened)
  const [token, setToken] = useState('')
  const [copied, setCopied] = useState(false)
  useFreshPairing(api, state, run)
  const link = state?.link
  const bot = state?.bot
  return (
    <ChatAppRow app="telegram" name="Telegram" state={state} open={open} onToggle={() => setOpen(!open)} failure={failure}>
      {bot === undefined && (
        <div className="lc-chatapp__steps" data-chat-step="token">
          <p className="lc-settings__note">1. In Telegram, open @BotFather and send /newbot. Give your bot any name; BotFather answers with its token.</p>
          <span><button type="button" className="lc-control lc-control--boxed" onClick={() => openLink(BOTFATHER_LINK)}>Open BotFather</button></span>
          <p className="lc-settings__note">2. Paste the token here. It is kept encrypted on this computer and sent only to Telegram.</p>
          <form className="lc-chatapp__token" onSubmit={(event) => {
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

      {bot !== undefined && state?.chat === undefined && (
        <div className="lc-phone" data-chat-step="pair">
          {link !== undefined && <QrCode text={link.url} label="A code for your phone’s camera. It opens your bot in Telegram to pair your chat." />}
          <div className="lc-phone__side">
            <p className="lc-settings__note" role="status">{state?.connected === true ? `Reading @${bot.username}’s messages.` : 'Connecting to Telegram…'}</p>
            {link !== undefined
              ? <>
                <p className="lc-settings__note">3. Scan the code with your phone, or open the link here, and press Start. It pairs one chat, once, within ten minutes.</p>
                <span className="lc-chatapp__buttons">
                  <button type="button" className="lc-control lc-control--boxed" onClick={() => openLink(link.url)}>Open in Telegram</button>
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

      {bot !== undefined && state?.chat !== undefined && (
        <>
          <ul className="lc-phone__devices" aria-label="Your Telegram chat">
            <li className="lc-phone__device">
              <span className="lc-phone__who">
                <span className="lc-phone__name">{state.chat.name}{state.chat.username === undefined ? '' : ` (@${state.chat.username})`}</span>
                <span className="lc-phone__state">{state.connected ? `Paired with @${bot.username}` : `Paired with @${bot.username} · not connected now`}</span>
              </span>
              <button type="button" className="lc-button" disabled={busy} onClick={() => { void run(() => api!.newLink()) }}>Pair another chat</button>
              <button type="button" className="lc-button" disabled={busy} onClick={() => { void run(() => api!.remove()) }}>Remove</button>
            </li>
          </ul>
          {link !== undefined && (
            <div className="lc-phone" data-chat-step="pair-again">
              <QrCode text={link.url} label="A code for your phone’s camera. It pairs another Telegram chat in place of this one." />
              <div className="lc-phone__side">
                <p className="lc-settings__note">The chat that presses Start with this link takes the place of {state.chat.name}’s, within ten minutes.</p>
                <span><button type="button" className="lc-control lc-control--boxed" onClick={() => openLink(link.url)}>Open in Telegram</button></span>
              </div>
            </div>
          )}
          <p className="lc-settings__note">In Telegram, write /name and a message to @{bot.username}. Telegram’s servers can read what you and your teammates write there: a bot’s chat is not end-to-end encrypted. Locust takes out anything shaped like a key before it sends, but keep secrets out of it. Approval cards there offer Allow once and Deny; Always stays here.</p>
        </>
      )}
    </ChatAppRow>
  )
}
